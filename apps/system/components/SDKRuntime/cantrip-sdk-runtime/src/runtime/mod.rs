// Copyright 2022 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// TODO(sleffler): model_wait & timer_wait by one application
//   potentially blocks all others

use cfg_if::cfg_if;

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use bitvec::prelude::*;
use cantrip_os_common::camkes::seL4_CPath;
use cantrip_os_common::cspace_slot::CSpaceSlot;
use cantrip_os_common::sel4_sys;
use cantrip_sdk_manager::SDKManagerError;
use cantrip_sdk_manager::SDKManagerInterface;
use cantrip_security_interface::cantrip_security_delete_key;
use cantrip_security_interface::cantrip_security_read_key;
use cantrip_security_interface::cantrip_security_write_key;
use core::hash::BuildHasher;
use hashbrown::HashMap;
cfg_if! {
    if #[cfg(feature = "ml_support")] {
        use cantrip_ml_interface::cantrip_mlcoord_cancel;
        use cantrip_ml_interface::cantrip_mlcoord_oneshot;
        use cantrip_ml_interface::cantrip_mlcoord_periodic;
        use cantrip_ml_interface::cantrip_mlcoord_poll;
        use cantrip_ml_interface::cantrip_mlcoord_wait;
        use cantrip_ml_interface::cantrip_mlcoord_get_output;
        use cantrip_ml_interface::cantrip_mlcoord_get_input_params;
        use cantrip_ml_interface::cantrip_mlcoord_set_input;
        use cantrip_ml_interface::MlCoordError;
    }
}
cfg_if! {
    if #[cfg(feature = "timer_support")] {
        use cantrip_timer_interface::cantrip_timer_cancel;
        use cantrip_timer_interface::cantrip_timer_oneshot;
        use cantrip_timer_interface::cantrip_timer_periodic;
        use cantrip_timer_interface::cantrip_timer_poll;
        use cantrip_timer_interface::cantrip_timer_wait;
        use cantrip_timer_interface::TimerServiceError;
    }
}
use log::{info, trace};
use sdk_interface::error::SDKError;
use sdk_interface::KeyValueData;
use sdk_interface::ModelId;
use sdk_interface::ModelInput;
use sdk_interface::ModelMask;
use sdk_interface::ModelOutput;
use sdk_interface::SDKAppId;
use sdk_interface::SDKRuntimeInterface;
use sdk_interface::TimerDuration;
use sdk_interface::TimerId;
use sdk_interface::TimerMask;
use smallstr::SmallString;
use smallvec::SmallVec;

use sel4_sys::seL4_CPtr;
use sel4_sys::seL4_CapRights;

// App capacity before spillover to the heap; should be the max concurrent
// started apps. Set very small because we expect, at least initially, that
// only one app at a time will be started.
const DEFAULT_APP_CAPACITY: usize = 3;

// BundleId capacity before spillover to the heap.
// TODO(sleffler): hide this; it's part of the implementation
// TODO(sleffler): shared with cantrip-proc-interface
const DEFAULT_BUNDLE_ID_CAPACITY: usize = 64;
type SmallId = SmallString<[u8; DEFAULT_BUNDLE_ID_CAPACITY]>;

// Each application gets timer & model id's in the range [0..31].
// There is one active model at any time and it is assigned the
// last valid id. Timer id's are mapped between application-assigned
// values and the global SDKRuntime id space. It is possible to exhaust
// the SDKRuntime id space (since it is only 32).
const MODEL_ID: ModelId = 31;
// Max TimerId an application can use.
const MAX_TIMER_ID: TimerId = (MODEL_ID - 1) as TimerId;

// Size of audio recording buffer. This holds samples to be returned to
// a user via an |audio_record_collect| call. The buffer is allocated on
// the heap while actively recording.
const AUDIO_RECORD_CAPACITY: usize = 1024; // XXX maybe match i2s::buffer::BUFFER_CAPACITY

#[allow(dead_code)]
#[derive(PartialEq)]
enum TimerState {
    None,
    Oneshot(TimerId),
    Periodic(TimerId),
}
impl TimerState {
    #[allow(dead_code)]
    pub fn get_id(&self) -> Option<TimerId> {
        match self {
            TimerState::None => None,
            TimerState::Oneshot(id) => Some(*id),
            TimerState::Periodic(id) => Some(*id),
        }
    }
}
const NO_TIMER: TimerState = TimerState::None; // NB: for initializing timer_state

#[allow(dead_code)]
#[derive(PartialEq)]
enum ModelState {
    None,
    Idle(String),    // Model may be loaded but not running
    Oneshot(String), // XXX maybe SmallString
    Periodic(String),
}
#[allow(dead_code)]
impl ModelState {
    pub fn get_name(&self) -> Option<&str> {
        match self {
            ModelState::None => None,
            ModelState::Idle(name) => Some(name),
            ModelState::Oneshot(name) => Some(name),
            ModelState::Periodic(name) => Some(name),
        }
    }
    pub fn is_idle(&self) -> bool { matches!(self, ModelState::Idle(_)) }
}

#[allow(dead_code)]
#[derive(PartialEq)]
enum AudioRecordState {
    Idle,
    Recording(Box<[u8; AUDIO_RECORD_CAPACITY]>),
}
#[allow(dead_code)]
impl AudioRecordState {
    pub fn is_idle(&self) -> bool { matches!(self, AudioRecordState::Idle) }
    pub fn is_recording(&self) -> bool { matches!(self, AudioRecordState::Recording(_)) }
    pub fn get_data(&self, max_data: usize) -> &[u8] {
        match self {
            AudioRecordState::Recording(data) => {
                &data[..core::cmp::min(max_data, AUDIO_RECORD_CAPACITY)]
            }
            _ => unimplemented!(),
        }
    }
    pub fn get_data_mut(&mut self, max_data: usize) -> &mut [u8] {
        match self {
            AudioRecordState::Recording(data) => {
                &mut data[..core::cmp::min(max_data, AUDIO_RECORD_CAPACITY)]
            }
            _ => unimplemented!(),
        }
    }
}

#[allow(dead_code)]
#[derive(PartialEq)]
enum AudioPlayState {
    Idle,
    Playing,
}
#[allow(dead_code)]
impl AudioPlayState {
    pub fn is_idle(&self) -> bool { matches!(self, AudioPlayState::Idle) }
    pub fn is_playing(&self) -> bool { matches!(self, AudioPlayState::Playing) }
}

// Per-app runtime state (mostly)  for tracking asynchronous activities:
// running models and timers. Only one running model is supported. Up to
// MAX_TIMER_ID timers may active but timers are shared betweenn applications
// so fewer may be available at any one time. Each application has it's own
// timer id space that is mapped into the id space of the runtime. The one
// model is always associated with id |MODEL_ID| which is MAX_TIMER_ID+1.
struct SDKRuntimeState {
    app_id: SmallId,
    model_state: ModelState,
    audio_record_state: AudioRecordState,
    audio_play_state: AudioPlayState,
    timer_state: [TimerState; MAX_TIMER_ID as usize + 1],
    // Bitmask of runtime timer id's; use native bit order because the
    // underlying u32 is used directly in timer_wait & timer_poll.
    sdk_timer_mask: BitArray<[u32; 1], Lsb0>,
}
impl SDKRuntimeState {
    // Allocates a runtime state instance for application |app_id|.
    pub fn new(app_id: &str) -> Self {
        Self {
            app_id: SmallId::from_str(app_id),
            model_state: ModelState::None,
            audio_record_state: AudioRecordState::Idle,
            audio_play_state: AudioPlayState::Idle,
            timer_state: [NO_TIMER; MAX_TIMER_ID as usize + 1],
            sdk_timer_mask: BitArray::ZERO,
        }
    }

    #[cfg(feature = "timer_support")]
    // Sets timer |app_timer_id| state to |state|.
    pub fn set_state(&mut self, app_id: TimerId, state: TimerState) {
        if let Some(timer_id) = state.get_id() {
            self.sdk_timer_mask.set(timer_id as usize, true);
        }
        self.timer_state[app_id as usize] = state;
    }

    #[cfg(feature = "timer_support")]
    // Clears state for timer |app_timer_id|.
    pub fn clr_state(&mut self, app_timer_id: TimerId) {
        if let Some(sdk_timer_id) = self.get_mapping(app_timer_id) {
            self.sdk_timer_mask.set(sdk_timer_id as usize, false);
            self.timer_state[app_timer_id as usize] = TimerState::None;
        }
    }

    #[cfg(feature = "timer_support")]
    // Returns any runtime timer id for |app_timer_id|. This is used to map
    // a timer id in the appllcation's timer space any sdk timer id
    pub fn get_mapping(&self, app_timer_id: TimerId) -> Option<TimerId> {
        self.timer_state[app_timer_id as usize].get_id()
    }

    #[cfg(feature = "timer_support")]
    // Returns an iterator that enumerates active runtime timers.
    pub fn timer_id_iter(&self) -> impl Iterator<Item = TimerId> + '_ {
        self.timer_state.iter().filter_map(|s| s.get_id())
    }

    #[cfg(feature = "ml_support")]
    // Processes a mask of completed ML jobs. This is simple atm because
    // at most one model may be loaded at a time and we fix the model id
    // (and ignore multiple apps running simultaneously)..
    pub fn process_completed_jobs(&mut self, mask: ModelMask) -> ModelMask {
        if (mask & (1 << MODEL_ID)) != 0 {
            if let ModelState::Oneshot(name) = &self.model_state {
                // XXX is this safe or do we need to go to None;
                // the latter would require doing a get_input_params
                // before every or using a model name instead of id
                self.model_state = ModelState::Idle(name.clone());
            }
        }
        mask
    }
}

/// Kata OS SDK support for third-party applications,
///
/// This is the server-side implementation. There is (currently) one thread
/// servicing multiple applications which causes us to multiplex / map
/// certain resources (e.g. the TimerService supports at most 32 timers
/// that we share among all applications). The runtime mostly serves as
/// a proxy for applications to other KataOS system services. But it also
/// provides a unified interface for waiting/polling asynchronous activities
/// by combining event notifications into a single api.
///
/// The SDKRuntime also includes the SDKManager that handles endpoint
/// minting for applications. When the ProcessManager starts an application
/// it requests an endpoint capability that is stored in the application's
/// top-level CNode. The slot number is then passed to the crt0 which writes
/// it to a global variable in the application's address space to when
/// sending SDK RPC's to the runtime (see the sdk-interface crate).
// XXX hashmap may be overkill, could use SmallVec and badge by index
pub struct SDKRuntime {
    endpoint: seL4_CPath,
    apps: HashMap<SDKAppId, SDKRuntimeState>,
    ids: BitArray<[u32; 1], Lsb0>, // Pool of global timer+model id's
    pending_mask: u32,             // Bitmask of undelivered events
}
impl SDKRuntime {
    pub fn new(endpoint: &seL4_CPath) -> Self {
        Self {
            endpoint: *endpoint,
            apps: HashMap::with_capacity(DEFAULT_APP_CAPACITY),
            ids: BitArray::ZERO,
            pending_mask: 0,
        }
    }

    // Calculates the badge assigned to the seL4 endpoint the client will use
    // to send requests to the SDKRuntime. This must be unique among active
    // clients but may be reused. There is no need to randomize or otherwise
    // secure this value since clients cannot forge an endpoint.
    // TODO(sleffler): is it worth doing a hash? counter is probably sufficient
    #[cfg(target_pointer_width = "32")]
    fn calculate_badge(&self, id: &SmallId) -> SDKAppId {
        (self.apps.hasher().hash_one(id) & 0x0ffffff) as SDKAppId
    }

    #[cfg(target_pointer_width = "64")]
    fn calculate_badge(&self, id: &SmallId) -> SDKAppId {
        self.apps.hasher().hash_one(id) as SDKAppId
    }

    pub fn capacity(&self) -> usize { self.apps.capacity() }

    // Wrappers that check for a valid client badge.
    fn get_app(&self, app_id: SDKAppId) -> Result<&SDKRuntimeState, SDKError> {
        self.apps.get(&app_id).ok_or(SDKError::InvalidBadge)
    }
    fn get_mut_app(&mut self, app_id: SDKAppId) -> Result<&mut SDKRuntimeState, SDKError> {
        self.apps.get_mut(&app_id).ok_or(SDKError::InvalidBadge)
    }

    #[cfg(feature = "timer_support")]
    // Allocates a timer id in the runtime time space.
    fn alloc_id(&mut self) -> Option<TimerId> {
        let bits = self.ids.as_mut_bitslice();
        let id = bits.first_zero()?;
        unsafe { bits.set_unchecked(id, true) };
        Some(id as TimerId)
    }

    #[cfg(feature = "timer_support")]
    // Releases a runtime timer id previously allocated with alloc_id.
    fn release_id(&mut self, id: TimerId) {
        self.ids.set(id as usize, false);
        self.pending_mask &= !(1 << id); // Discard any pending notification
    }

    #[cfg(feature = "timer_support")]
    // Process completed timers: reclaim oneshot timer id's and returns the
    // the mask of application timer id's.
    // NB: potentially espensive
    fn process_completed_timers(
        &mut self,
        app_id: SDKAppId,
        mut sdk_timer_mask: TimerMask, // Mask of runtime timer id's
    ) -> Result<TimerMask, SDKError> {
        assert!(sdk_timer_mask != 0);

        // Calculate the mask of app timer id's and identify any oneshot
        // timers. Note we clear state separately to appease the borrows
        // checker.
        // NB: we use u8's to conserve stack space
        let mut sdk_oneshots = SmallVec::<[u8; MAX_TIMER_ID as usize + 1]>::new();
        let mut app_oneshots = SmallVec::<[u8; MAX_TIMER_ID as usize + 1]>::new();

        let app = self.get_mut_app(app_id)?;
        let mut app_mask = 0;
        for (app_id, state) in app.timer_state.iter().enumerate() {
            if let Some(sdk_id) = state.get_id() {
                if (sdk_timer_mask & (1 << sdk_id)) != 0 {
                    app_mask |= 1 << app_id;
                    if let TimerState::Oneshot(_) = *state {
                        app_oneshots.push(app_id as u8);
                        sdk_oneshots.push(sdk_id as u8);
                    }
                    sdk_timer_mask &= !(1 << sdk_id);
                    if sdk_timer_mask == 0 {
                        break;
                    }
                }
            }
        }

        // Release oneshot timer state.
        while let Some(app_id) = app_oneshots.pop() {
            app.clr_state(app_id as TimerId);
        }
        while let Some(sdk_id) = sdk_oneshots.pop() {
            self.release_id(sdk_id as TimerId);
        }
        Ok(app_mask)
    }
}
impl SDKManagerInterface for SDKRuntime {
    /// Returns an seL4 Endpoint capability for |app_id| to make SDKRuntime
    /// requests. All requests will fail without first calling
    /// cantrip_sdk_manager_get_endpoint().
    fn get_endpoint(&mut self, app_id: &str) -> Result<seL4_CPtr, SDKManagerError> {
        let badge = self.calculate_badge(&SmallId::from_str(app_id));

        // Mint a badged endpoint for the client to talk to us.
        let mut slot = CSpaceSlot::new();
        slot.mint_to(
            self.endpoint.0,
            self.endpoint.1,
            self.endpoint.2 as u8,
            seL4_CapRights::new(
                /*grant_reply=*/ 1,
                /*grant=*/ 1, // NB: to send frame with RPC params
                /*read=*/ 0, /*write=*/ 1,
            ),
            badge,
        )
        .or(Err(SDKManagerError::GetEndpointFailed))?;

        // Create the entry & return the endpoint capability.
        assert!(self
            .apps
            .insert(badge, SDKRuntimeState::new(app_id))
            .is_none());
        Ok(slot.release())
    }

    /// Releases |app_id| state. No future requests may be made without
    /// first calling cantrip_sdk_manager_get_endpoint().
    #[allow(unused_variables)]
    fn release_endpoint(&mut self, app_id: &str) -> Result<(), SDKManagerError> {
        let badge = self.calculate_badge(&SmallId::from_str(app_id));
        if let Some(app) = self.apps.remove(&badge) {
            // Cleanup app timer & model state.
            #[cfg(feature = "ml_support")]
            if let Some(name) = app.model_state.get_name() {
                let _ = cantrip_mlcoord_cancel(app_id, name);
                self.pending_mask &= !(1 << MODEL_ID);
            }
            #[cfg(feature = "timer_support")]
            for timer_id in app.timer_id_iter() {
                let _ = cantrip_timer_cancel(timer_id);
                self.release_id(timer_id);
            }
            #[cfg(feature = "audio_support")]
            {
                let _ = i2s_driver::audio_reset(
                    /*rxrst=*/ true, /*txrst=*/ true, /*rxilvl=*/ 1,
                    /*txilvl=*/ 1,
                );
            }
        } else {
            // NB: assumed to be compiled out in release build (no DDOS).
            trace!("release of nonexistent endpoint {}", app_id);
        }
        Ok(())
    }
}
impl SDKRuntimeInterface for SDKRuntime {
    /// Pings the SDK runtime, going from client to server and back via CAmkES IPC.
    fn ping(&self, app_id: SDKAppId) -> Result<(), SDKError> {
        match self.apps.get(&app_id) {
            Some(_) => Ok(()),
            None => {
                // NB: assumed to be compiled out in release build (no DDOS).
                trace!("no entry for app_id {:x}", app_id);
                Err(SDKError::InvalidBadge)
            }
        }
    }

    /// Logs |msg| through the system logger.
    fn log(&self, app_id: SDKAppId, msg: &str) -> Result<(), SDKError> {
        let app = self.get_app(app_id)?;
        // NB: app can use this to overflow the heap
        info!(target: &alloc::format!("[{}]", app.app_id), "{}", msg);
        Ok(())
    }

    /// Returns any value for the specified |key| in the app's  private key-value store.
    fn read_key(&self, app_id: SDKAppId, key: &str) -> Result<KeyValueData, SDKError> {
        let app = self.get_app(app_id)?;
        cantrip_security_read_key(&app.app_id, key).or(Err(SDKError::ReadKeyFailed))
    }

    /// Writes |value| for the specified |key| in the app's private key-value store.
    fn write_key(&self, app_id: SDKAppId, key: &str, value: &KeyValueData) -> Result<(), SDKError> {
        let app = self.get_app(app_id)?;
        cantrip_security_write_key(&app.app_id, key, value).or(Err(SDKError::WriteKeyFailed))?; // XXX
        Ok(())
    }

    /// Deletes the specified |key| in the app's private key-value store.
    fn delete_key(&self, app_id: SDKAppId, key: &str) -> Result<(), SDKError> {
        let app = self.get_app(app_id)?;
        cantrip_security_delete_key(&app.app_id, key).or(Err(SDKError::DeleteKeyFailed))?; // XXX
        Ok(())
    }

    #[allow(unused_variables)]
    fn timer_oneshot(
        &mut self,
        app_id: SDKAppId,
        id: TimerId,
        duration_ms: TimerDuration,
    ) -> Result<(), SDKError> {
        trace!("timer_oneshot id {} duration {}", id, duration_ms);
        // NB: cannot hold mutable ref over alloc_id call
        let _ = self.get_app(app_id)?;
        if id > MAX_TIMER_ID {
            return Err(SDKError::NoSuchTimer);
        }
        #[cfg(feature = "timer_support")]
        {
            let timer_id = self.alloc_id().ok_or(SDKError::OutOfResources)?;
            if let Err(e) = cantrip_timer_oneshot(timer_id, duration_ms) {
                self.release_id(timer_id);
                return Err(map_timer_err(e));
            }
            unsafe { self.get_mut_app(app_id).unwrap_unchecked() }
                .set_state(id, TimerState::Oneshot(timer_id));
            Ok(())
        }

        #[cfg(not(feature = "timer_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    #[allow(unused_variables)]
    fn timer_periodic(
        &mut self,
        app_id: SDKAppId,
        id: TimerId,
        duration_ms: TimerDuration,
    ) -> Result<(), SDKError> {
        trace!("timer_periodic id {} duration {}", id, duration_ms);
        // NB: cannot hold mutable ref over alloc_id call
        let _ = self.get_app(app_id)?;
        if id > MAX_TIMER_ID {
            return Err(SDKError::NoSuchTimer);
        }
        #[cfg(feature = "timer_support")]
        {
            let timer_id = self.alloc_id().ok_or(SDKError::OutOfResources)?;
            if let Err(e) = cantrip_timer_periodic(timer_id, duration_ms) {
                self.release_id(timer_id);
                return Err(map_timer_err(e));
            }
            // NB: cannot hold mutable ref over alloc_id call
            unsafe { self.get_mut_app(app_id).unwrap_unchecked() }
                .set_state(id, TimerState::Periodic(timer_id));
            Ok(())
        }

        #[cfg(not(feature = "timer_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    #[allow(unused_variables)]
    fn timer_cancel(&mut self, app_id: SDKAppId, id: TimerId) -> Result<(), SDKError> {
        trace!("timer_cancel id {}", id);
        let app = self.get_mut_app(app_id)?;
        if id > MAX_TIMER_ID {
            return Err(SDKError::NoSuchTimer);
        }
        #[cfg(feature = "timer_support")]
        {
            let timer_id = app.get_mapping(id).ok_or(SDKError::InvalidTimer)?;
            // TODO(sleffler): selectively ignore errors?
            let _ = cantrip_timer_cancel(timer_id);
            app.clr_state(id);
            self.release_id(timer_id);
            Ok(())
        }

        #[cfg(not(feature = "timer_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    #[allow(unused_variables)]
    fn timer_wait(&mut self, app_id: SDKAppId) -> Result<TimerMask, SDKError> {
        trace!("timer_wait");
        #[cfg(feature = "timer_support")]
        {
            let mut ret_mask;
            loop {
                ret_mask = self.get_app(app_id)?.sdk_timer_mask.into_inner()[0];
                if ret_mask == 0 {
                    // No pending timers for app.
                    break;
                }
                // Check for pending events.
                if (self.pending_mask & ret_mask) == 0 {
                    // XXX this is blocking
                    self.pending_mask |= cantrip_timer_wait().map_err(map_timer_err)?;
                }
                // Calculate app's events & subtract those from the pending set.
                ret_mask &= self.pending_mask;
                self.pending_mask &= !ret_mask;
                if ret_mask != 0 {
                    // NB:: converts runtime id mask to app id mask for return.
                    ret_mask = self.process_completed_timers(app_id, ret_mask)?;
                    break;
                }
            }
            Ok(ret_mask)
        }

        #[cfg(not(feature = "timer_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    #[allow(unused_variables)]
    fn timer_poll(&mut self, app_id: SDKAppId) -> Result<TimerMask, SDKError> {
        trace!("timer_poll");
        let mut ret_mask = self.get_mut_app(app_id)?.sdk_timer_mask.into_inner()[0];
        #[cfg(feature = "timer_support")]
        {
            if ret_mask != 0 {
                if (self.pending_mask & ret_mask) == 0 {
                    self.pending_mask |= cantrip_timer_poll().map_err(map_timer_err)?;
                }
                ret_mask &= self.pending_mask;
                self.pending_mask &= !ret_mask;
                if ret_mask != 0 {
                    // NB:: converts runtime id mask to app id mask for return.
                    ret_mask = self.process_completed_timers(app_id, ret_mask)?;
                }
            }
            Ok(ret_mask)
        }

        #[cfg(not(feature = "timer_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    #[allow(unused_variables)]
    fn model_oneshot(&mut self, app_id: SDKAppId, model_id: &str) -> Result<ModelId, SDKError> {
        trace!("model_oneshot {}", model_id);
        let app = self.get_mut_app(app_id)?;
        #[cfg(feature = "ml_support")]
        {
            cantrip_mlcoord_oneshot(&app.app_id, model_id).map_err(map_ml_err)?;
            app.model_state = ModelState::Oneshot(model_id.into());
            Ok(MODEL_ID)
        }

        #[cfg(not(feature = "ml_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    #[allow(unused_variables)]
    fn model_periodic(
        &mut self,
        app_id: SDKAppId,
        model_id: &str,
        duration_ms: TimerDuration,
    ) -> Result<ModelId, SDKError> {
        trace!("model_periodic {} duration {}", model_id, duration_ms);
        let app = self.get_mut_app(app_id)?;
        #[cfg(feature = "ml_support")]
        {
            cantrip_mlcoord_periodic(&app.app_id, model_id, duration_ms).map_err(map_ml_err)?;
            app.model_state = ModelState::Periodic(model_id.into());
            Ok(MODEL_ID)
        }

        #[cfg(not(feature = "ml_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    fn model_cancel(&mut self, app_id: SDKAppId, id: ModelId) -> Result<(), SDKError> {
        trace!("model_cancel {}", id);
        let app = self.get_mut_app(app_id)?;
        if id != MODEL_ID {
            return Err(SDKError::NoSuchModel);
        }
        if app.model_state == ModelState::None {
            return Ok(()); // XXX error?
        }
        #[cfg(feature = "ml_support")]
        {
            cantrip_mlcoord_cancel(&app.app_id, app.model_state.get_name().unwrap())
                .map_err(map_ml_err)?;
            // XXX Idle?
            app.model_state = ModelState::None;
            Ok(())
        }

        #[cfg(not(feature = "ml_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    fn model_wait(&mut self, app_id: SDKAppId) -> Result<ModelMask, SDKError> {
        trace!("model_wait");
        let app = self.get_mut_app(app_id)?;
        if app.model_state == ModelState::None {
            return Ok(0); // Nothing running
        }
        #[cfg(feature = "ml_support")]
        {
            // XXX blocking
            cantrip_mlcoord_wait()
                .map_err(map_ml_err)
                .map(|mask| app.process_completed_jobs(mask))
        }

        #[cfg(not(feature = "ml_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    fn model_poll(&mut self, app_id: SDKAppId) -> Result<ModelMask, SDKError> {
        trace!("model_poll");
        let app = self.get_mut_app(app_id)?;
        if app.model_state == ModelState::None {
            return Ok(0); // Nothing running
        }
        #[cfg(feature = "ml_support")]
        {
            cantrip_mlcoord_poll()
                .map_err(map_ml_err)
                .map(|mask| app.process_completed_jobs(mask))
        }

        #[cfg(not(feature = "ml_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    fn model_output(&mut self, app_id: SDKAppId, id: ModelId) -> Result<ModelOutput, SDKError> {
        trace!("model_output {}", id);
        let app = self.get_mut_app(app_id)?;
        if id != MODEL_ID {
            return Err(SDKError::NoSuchModel);
        }
        if app.model_state == ModelState::None {
            return Err(SDKError::NoSuchModel);
        }
        #[cfg(feature = "ml_support")]
        {
            cantrip_mlcoord_get_output(&app.app_id, app.model_state.get_name().unwrap())
                .map_err(map_ml_err)
                .map(|output| ModelOutput {
                    jobnum: output.jobnum,
                    return_code: output.return_code,
                    epc: output.epc,
                    data: output.data,
                })
        }

        #[cfg(not(feature = "ml_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    #[allow(unused_variables)]
    fn model_get_input_params(
        &mut self,
        app_id: SDKAppId,
        model_id: &str,
    ) -> Result<(ModelId, ModelInput), SDKError> {
        trace!("model_get_input_params {}", model_id);
        let app = self.get_mut_app(app_id)?;
        #[cfg(feature = "ml_support")]
        {
            let mlinput =
                cantrip_mlcoord_get_input_params(&app.app_id, model_id).map_err(map_ml_err)?;
            app.model_state = ModelState::Idle(model_id.into());
            Ok((
                MODEL_ID,
                ModelInput {
                    input_ptr: mlinput.input_ptr,
                    input_size_bytes: mlinput.input_size_bytes,
                },
            ))
        }

        #[cfg(not(feature = "ml_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    #[allow(unused_variables)]
    fn model_set_input(
        &mut self,
        app_id: SDKAppId,
        id: ModelId,
        input_data_offset: u32,
        input_data: &[u8],
    ) -> Result<(), SDKError> {
        trace!("model_set_input {id} {input_data_offset} {:x?}", input_data);
        let app = self.get_mut_app(app_id)?;
        if id != MODEL_ID {
            return Err(SDKError::NoSuchModel);
        }
        // Require model to be loaded+stopped; this may be too conservative.
        if !app.model_state.is_idle() {
            return Err(SDKError::NoSuchModel); // XXX need is running error
        }
        #[cfg(feature = "ml_support")]
        {
            cantrip_mlcoord_set_input(
                &app.app_id,
                app.model_state.get_name().unwrap(),
                input_data_offset,
                input_data,
            )
            .map_err(map_ml_err)
        }

        #[cfg(not(feature = "ml_support"))]
        Err(SDKError::NoPlatformSupport)
    }

    #[allow(unused_variables)]
    fn audio_reset(
        &mut self,
        app_id: SDKAppId,
        rxrst: bool,
        txrst: bool,
        rxilvl: u8,
        txilvl: u8,
    ) -> Result<(), SDKError> {
        trace!("audio_reset rx {rxrst} {rxilvl} tx {txrst} {txilvl}");
        let app = self.get_mut_app(app_id)?;
        cfg_if! {
            if #[cfg(feature = "audio_support")] {
                i2s_driver::audio_reset(rxrst, txrst, rxilvl, txilvl)?;
                if rxrst {
                    app.audio_record_state = AudioRecordState::Idle;
                }
                if txrst {
                    app.audio_play_state = AudioPlayState::Idle;
                }
                Ok(())
            } else {
                Err(SDKError::NoPlatformSupport)
            }
        }
    }
    #[allow(unused_variables)]
    fn audio_record_start(
        &mut self,
        app_id: SDKAppId,
        rate: usize,
        buffer_size: usize,
        stop_on_full: bool,
    ) -> Result<(), SDKError> {
        trace!("audio_record_start {rate} {buffer_size} {stop_on_full}");
        let app = self.get_mut_app(app_id)?;
        cfg_if! {
            if #[cfg(feature = "audio_support")] {
                i2s_driver::audio_record_start(rate, buffer_size, stop_on_full)?;
                // XXX buffer_size
                // XXX new_uninit
                app.audio_record_state = AudioRecordState::Recording(Box::new([0u8; AUDIO_RECORD_CAPACITY]));
                Ok(())
            } else {
                Err(SDKError::NoPlatformSupport)
            }
        }
    }
    #[allow(unused_variables)]
    fn audio_record_collect(
        &mut self,
        app_id: SDKAppId,
        max_data: usize,
        wait_if_empty: bool,
    ) -> Result<&[u8], SDKError> {
        trace!("audio_record_collect {max_data}");
        let app = self.get_mut_app(app_id)?;
        if !app.audio_record_state.is_recording() {
            return Err(SDKError::InvalidAudioState);
        }
        cfg_if! {
            if #[cfg(feature = "audio_support")] {
                let data = app.audio_record_state.get_data_mut(max_data);
                // XXX pin?
                let count = i2s_driver::audio_record_collect(data, wait_if_empty)?;
                Ok(&data[..count])
            } else {
                Err(SDKError::NoPlatformSupport)
            }
        }
    }
    #[allow(unused_variables)]
    fn audio_record_stop(&mut self, app_id: SDKAppId) -> Result<(), SDKError> {
        trace!("audio_record_stop");
        let app = self.get_mut_app(app_id)?;
        cfg_if! {
            if #[cfg(feature = "audio_support")] {
                i2s_driver::audio_record_stop()?;
                app.audio_record_state = AudioRecordState::Idle;
                Ok(())
            } else {
                Err(SDKError::NoPlatformSupport)
            }
        }
    }

    #[allow(unused_variables)]
    fn audio_play_start(
        &mut self,
        app_id: SDKAppId,
        rate: usize,
        buffer_size: usize,
    ) -> Result<(), SDKError> {
        trace!("audio_play_start {rate} {buffer_size}");
        let app = self.get_mut_app(app_id)?;
        cfg_if! {
            if #[cfg(feature = "audio_support")] {
                i2s_driver::audio_play_start(rate, buffer_size)?;
                app.audio_play_state = AudioPlayState::Playing;
                Ok(())
            } else {
                Err(SDKError::NoPlatformSupport)
            }
        }
    }
    #[allow(unused_variables)]
    fn audio_play_write(&mut self, app_id: SDKAppId, data: &[u8]) -> Result<(), SDKError> {
        trace!("audio_play_write {}", data.len());
        let app = self.get_mut_app(app_id)?;
        if !app.audio_play_state.is_playing() {
            return Err(SDKError::InvalidAudioState);
        }
        cfg_if! {
            if #[cfg(feature = "audio_support")] {
                // XXX async + double-buffer?
                i2s_driver::audio_play_write(data)
            } else {
                Err(SDKError::NoPlatformSupport)
            }
        }
    }
    #[allow(unused_variables)]
    fn audio_play_stop(&mut self, app_id: SDKAppId) -> Result<(), SDKError> {
        trace!("audio_play_stop");
        let app = self.get_mut_app(app_id)?;
        cfg_if! {
            if #[cfg(feature = "audio_support")] {
                i2s_driver::audio_play_stop()?;
                app.audio_play_state = AudioPlayState::Idle;
                Ok(())
            } else {
                Err(SDKError::NoPlatformSupport)
            }
        }
    }
}

#[cfg(feature = "timer_support")]
fn map_timer_err(err: TimerServiceError) -> SDKError {
    match err {
        TimerServiceError::NoSuchTimer => SDKError::NoSuchTimer,
        TimerServiceError::DeserializeFailed => SDKError::DeserializeFailed,
        TimerServiceError::SerializeFailed => SDKError::SerializeFailed,
        TimerServiceError::TimerAlreadyExists => SDKError::TimerAlreadyExists,
        TimerServiceError::UnknownError => unreachable!(),
        TimerServiceError::Success => unreachable!(),
    }
}

#[cfg(feature = "ml_support")]
fn map_ml_err(err: MlCoordError) -> SDKError {
    match err {
        MlCoordError::NoSuchModel | MlCoordError::InvalidImage => SDKError::NoSuchModel,
        MlCoordError::InvalidTimer => SDKError::InvalidTimer,
        MlCoordError::LoadModelFailed => SDKError::LoadModelFailed,
        MlCoordError::NoModelSlotsLeft => SDKError::OutOfResources,
        MlCoordError::NoOutputHeader => SDKError::NoModelOutput,
        MlCoordError::SerializeError => SDKError::SerializeFailed,
        MlCoordError::DeserializeError => SDKError::DeserializeFailed,
        MlCoordError::UnknownError => unreachable!(),
        MlCoordError::Success => unreachable!(),
        MlCoordError::InvalidInputRange => SDKError::InvalidInputRange,
    }
}
