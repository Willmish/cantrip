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

//! CantripOS SDK application runtime interfaces.

#![cfg_attr(not(test), no_std)]

pub mod error;

pub use error::SDKError;
pub use error::SDKRuntimeError;

extern crate alloc;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};
use zerovec::ZeroVec;

use sel4_sys::seL4_CPtr;
use sel4_sys::seL4_Call;
use sel4_sys::seL4_MessageInfo;
use sel4_sys::seL4_PageBits;
use sel4_sys::seL4_SetCap;

const PAGE_SIZE: usize = 1 << seL4_PageBits;

// SDKRuntime client-side state setup by ProcessManager and crt0.
// TODO(sleffler): is 1 page enough? ProcessManager should probably have
//   SDKRuntime handle this
extern "C" {
    static CANTRIP_SDK_ENDPOINT: seL4_CPtr; // IPC connection to SDKRuntime
    static CANTRIP_SDK_FRAME: seL4_CPtr; // RPC parameters frame
    static CANTRIP_SDK_PARAMS: *mut u8; // Virtual address of CANTRIP_SDK_FRAME
}

// Size of the buffers used to pass serialized data. The data structure
// sizes are bounded by the single page (4K bytes) used to marshal & unmarshal
// parameters and also by their being allocated on the stack. We balance
// these against being able to handle large amounts of data.
// XXX do sensor frames need to be passed & are they too big?

// pub for server-side logic
pub const SDKRUNTIME_REQUEST_DATA_SIZE: usize = PAGE_SIZE / 2;

/// Application identity derived from seL4 Endpoint badge setup when
/// the application is started by ProcessManager.
///
/// NB: On 32-bit platforms the kernel truncates this to 28-bits;
///     on 64-bit platforms these are 64-bits.
pub type SDKAppId = usize;

// TODO(sleffler): temp constraint on value part of key-value pairs
// TOOD(sleffler): dup's security coordinator but we don't want a dependency
pub const KEY_VALUE_DATA_SIZE: usize = 100;
pub type KeyValueData = [u8; KEY_VALUE_DATA_SIZE];

// TOOD(sleffler): dup's mlcoordinator but we don't want a dependency
pub const MAX_OUTPUT_DATA: usize = 128;

/// Core api's

/// SDKRuntimeRequest::Ping
#[derive(Serialize, Deserialize)]
pub struct PingRequest {}

/// SDKRuntimeRequest::Log
#[derive(Serialize, Deserialize)]
pub struct LogRequest<'a> {
    pub msg: &'a [u8],
}

/// SecurityCoordinator key-value api's

/// SDKRuntimeRequest::ReadKey
#[derive(Serialize, Deserialize)]
pub struct ReadKeyRequest<'a> {
    pub key: &'a str,
}
#[derive(Serialize, Deserialize)]
pub struct ReadKeyResponse<'a> {
    pub value: &'a [u8],
}

/// SDKRuntimeRequest::WriteKey
#[derive(Serialize, Deserialize)]
pub struct WriteKeyRequest<'a> {
    pub key: &'a str,
    pub value: &'a [u8],
}

/// SDKRuntimeRequest::DeleteKey
#[derive(Serialize, Deserialize)]
pub struct DeleteKeyRequest<'a> {
    pub key: &'a str,
}

/// TimerService api's

pub type TimerId = u32;
pub type TimerDuration = u32;
pub type TimerMask = u32;

/// SDKRuntimeRequest::OneshotTimer and SDKRuntimeRequest::PeriodicTimer
#[derive(Serialize, Deserialize)]
pub struct TimerStartRequest {
    pub id: TimerId,
    pub duration_ms: TimerDuration,
}

/// SDKRuntimeRequest::CancelTimer
#[derive(Serialize, Deserialize)]
pub struct TimerCancelRequest {
    pub id: TimerId,
}

/// SDKRuntimeRequest::WaitForTimers and SDKRuntimeRequest::PollForTimers
#[derive(Serialize, Deserialize)]
pub struct TimerWaitRequest {}
#[derive(Serialize, Deserialize)]
pub struct TimerWaitResponse {
    pub mask: TimerMask,
}

/// MlCoordinator api's

pub type ModelId = u32;
pub type ModelMask = u32;
// TODO(sleffler): could alias TimerDuration

// NB: serde helper for arrays w/ >32 elements
//   c.f. https://github.com/serde-rs/serde/pull/1860
use serde_big_array::big_array;
big_array! { BigArray; }

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelOutput {
    pub jobnum: usize,
    pub return_code: u32,
    pub epc: Option<u32>,
    #[serde(with = "BigArray")]
    pub data: [u8; MAX_OUTPUT_DATA],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInput {
    pub input_ptr: u32,
    pub input_size_bytes: u32,
}

/// SDKRuntimeRequest::OneshotModel
#[derive(Serialize, Deserialize)]
pub struct ModelOneshotRequest<'a> {
    pub model_id: &'a str,
}
#[derive(Serialize, Deserialize)]
pub struct ModelStartResponse {
    pub id: ModelId,
}

/// SDKRuntimeRequest::PeriodicModel
#[derive(Serialize, Deserialize)]
pub struct ModelPeriodicRequest<'a> {
    pub model_id: &'a str,
    pub duration_ms: TimerDuration,
}
// NB: returns ModelStartResponse

/// SDKRuntimeRequest::CancelModel
#[derive(Serialize, Deserialize)]
pub struct ModelCancelRequest {
    pub id: ModelId,
}

/// SDKRuntimeRequest::WaitForModel and SDKRuntimeRequest::PollForModels
#[derive(Serialize, Deserialize)]
pub struct ModelWaitRequest {}
#[derive(Serialize, Deserialize)]
pub struct ModelWaitResponse {
    pub mask: ModelMask,
}

/// SDKRuntimeRequest::GetModelOutput
#[derive(Serialize, Deserialize)]
pub struct ModelOutputRequest {
    pub id: ModelId,
}
#[derive(Serialize, Deserialize)]
pub struct ModelOutputResponse {
    pub output: ModelOutput,
}

/// SDKRuntimeRequest::GetModelInputParams
#[derive(Serialize, Deserialize)]
pub struct ModelGetInputParamsRequest<'a> {
    pub model_id: &'a str,
}
#[derive(Serialize, Deserialize)]
pub struct ModelGetInputParamsResponse {
    pub id: ModelId,
    pub input_params: ModelInput,
}

/// SDKRuntimeRequest::SetModelInput
#[derive(Serialize, Deserialize)]
pub struct ModelSetInputRequest<'a> {
    pub id: ModelId,
    pub input_data_offset: u32,
    pub input_data: &'a [u8],
}

/// Audio api's

/// SDKRuntimeRequest::AudioReset
#[derive(Serialize, Deserialize)]
pub struct AudioResetRequest {
    pub rxrst: bool, // Reset rx
    pub txrst: bool, // Reset tx
    pub rxilvl: u8,  // RX fifo level (one of 1,4,8,16,30)
    pub txilvl: u8,  // TX fifo level (one of 1,4,8,16)
}

/// SDKRuntimeRequest::AudioRecordStart
#[derive(Serialize, Deserialize)]
pub struct AudioRecordStartRequest {
    pub rate: usize,
    pub buffer_size: usize,
    // If true, stop on buffer full, otherwise treat as a circular buffer
    pub stop_on_full: bool,
}

/// SDKRuntimeRequest::AudioRecordCollect
#[derive(Serialize, Deserialize)]
pub struct AudioRecordCollectRequest {
    pub max_samples: usize,
    pub wait_if_empty: bool, // XXX wait for fifo to reach level?
}
#[derive(Serialize, Deserialize)]
pub struct AudioRecordCollectResponse<'a> {
    #[serde(borrow)]
    pub data: ZeroVec<'a, u32>,
}

/// SDKRuntimeRequest::AudioRecordStop
#[derive(Serialize, Deserialize)]
pub struct AudioRecordStopRequest {}

/// SDKRuntimeRequest::AudioPlayStart
#[derive(Serialize, Deserialize)]
pub struct AudioPlayStartRequest {
    pub rate: usize,
    pub buffer_size: usize, // XXX in samples?
}

/// SDKRuntimeRequest::AudioPlayWrite
#[derive(Serialize, Deserialize)]
pub struct AudioPlayWriteRequest<'a> {
    #[serde(borrow)]
    pub data: ZeroVec<'a, u32>,
}

/// SDKRuntimeRequest::AudioPlayStop
#[derive(Serialize, Deserialize)]
pub struct AudioPlayStopRequest {}

/// SDKRequest token sent over the seL4 IPC interface. We need repr(seL4_Word)
/// but cannot use that so use the implied usize type instead.
///
/// Note that this enum starts off at 64. This is to avoid collisions with the
/// seL4_Fault enumeration used by the kernel, as the SDK runtime is also used
/// as the application's fault handler.
#[repr(usize)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum SDKRuntimeRequest {
    Ping = 64, // Check runtime is alive
    Log,       // Log message: [msg: &str]

    ReadKey,   // Read key: [key: &str, &mut [u8]] -> value: &[u8]
    WriteKey,  // Write key: [key: &str, value: &KeyValueData]
    DeleteKey, // Delete key: [key: &str]

    OneshotTimer,  // One-shot timer: [id: TimerId, duration_ms: TimerDuration]
    PeriodicTimer, // Periodic timer: [id: TimerId, duration_ms: TimerDuration]
    CancelTimer,   // Cancel timer: [id: TimerId]
    WaitForTimers, // Wait for timers to expire: [] -> TimerMask
    PollForTimers, // Poll for timers to expire: [] -> TimerMask

    OneshotModel,        // One-shot model execution: [model_id: &str] -> id: ModelId
    PeriodicModel, // Periodic model execution: [model_id: &str, duration_ms: TimerDuration] -> ModelId
    CancelModel,   // Cancel running model: [id: ModelId]
    WaitForModel,  // Wait for any running model to complete: [] -> ModelMask
    PollForModels, // Poll for running models to complete: [] -> ModelMask
    GetModelOutput, // Return output data from most recent run: [id: ModelId, clear: bool] -> ModelOutput
    GetModelInputParams, // Load model & return input data params: [model_id: &str] -> (ModelId, ModelInput)
    SetModelInput, // Set input data for loaded model: [id: ModelId, input_data_offset: u32, input_data: &[u8]

    AudioReset, // Reset audio state: [rxrst: bool, txrst: bool, rxilvl: u8, txilvl: u8]
    AudioRecordStart, // Start recording: [rate: usize, buffer_size: usize, stop_on_full: bool]
    AudioRecordCollect, // Collect recorded data: [max_samples: usize, wait_if_empty: bool]
    AudioRecordStop, // Stop recording (any un-collected data are discarded): []
    AudioPlayStart, // Start playing: [rate: usize, buffer_size: usize]
    AudioPlayWrite, // Write play samples: [data: &[u32]]
    AudioPlayStop, // Stop playing: []
}

/// Rust interface for the SDKRuntime.
///
/// This trait defines all of the same verbs we expect to support in the component
/// interface, for both client and server, since CAmkES does not (yet) know how
/// to generate Rust bindings.
///
/// On the server side, the impl of this trait is instantiated in the component
/// as a global mutable object where the incoming calls from the CAmkES C side
/// are wrapped.
///
/// On the client side, this trait is implemented using top-level functions.
pub trait SDKRuntimeInterface {
    /// Pings the SDK runtime, going from client to server and back via CAmkES IPC.
    fn ping(&self, app_id: SDKAppId) -> Result<(), SDKError>;

    /// Logs |msg| through the system logger.
    fn log(&self, app_id: SDKAppId, msg: &str) -> Result<(), SDKError>;

    /// Returns any value for the specified |key| in the app's  private key-value store.
    /// Data are written to |keyval| and returned as a slice.
    fn read_key(&self, app_id: SDKAppId, key: &str) -> Result<KeyValueData, SDKError>;

    /// Writes |value| for the specified |key| in the app's private key-value store.
    fn write_key(&self, app_id: SDKAppId, key: &str, value: &KeyValueData) -> Result<(), SDKError>;

    /// Deletes the specified |key| in the app's private key-value store.
    fn delete_key(&self, app_id: SDKAppId, key: &str) -> Result<(), SDKError>;

    /// Create a one-shot timer named |id| of |duration_ms|.
    fn timer_oneshot(
        &mut self,
        app_id: SDKAppId,
        id: TimerId,
        duration_ms: TimerDuration,
    ) -> Result<(), SDKError>;
    /// Create a periodic (repeating) timer named |id| of |duration_ms|.
    fn timer_periodic(
        &mut self,
        app_id: SDKAppId,
        id: TimerId,
        duration_ms: TimerDuration,
    ) -> Result<(), SDKError>;
    /// Cancel a previously created timer.
    fn timer_cancel(&mut self, app_id: SDKAppId, id: TimerId) -> Result<(), SDKError>;
    /// Wait for any running timer to complete.
    fn timer_wait(&mut self, app_id: SDKAppId) -> Result<TimerMask, SDKError>;
    /// Poll for any running timer that have completed.
    fn timer_poll(&mut self, app_id: SDKAppId) -> Result<TimerMask, SDKError>;

    /// Create a one-shot run of |model_id|.
    fn model_oneshot(&mut self, app_id: SDKAppId, model_id: &str) -> Result<ModelId, SDKError>;
    /// Create a periodic (repeating) timer named |id| of |duration_ms|.
    fn model_periodic(
        &mut self,
        app_id: SDKAppId,
        model_id: &str,
        duration_ms: TimerDuration,
    ) -> Result<ModelId, SDKError>;
    /// Cancel a previously created timer.
    fn model_cancel(&mut self, app_id: SDKAppId, id: ModelId) -> Result<(), SDKError>;
    /// Wait for any running timer to complete.
    fn model_wait(&mut self, app_id: SDKAppId) -> Result<ModelMask, SDKError>;
    /// Poll for any running timer that have completed.
    fn model_poll(&mut self, app_id: SDKAppId) -> Result<ModelMask, SDKError>;
    /// Retrieve the output from the last run of model |id|.
    fn model_output(&mut self, app_id: SDKAppId, id: ModelId) -> Result<ModelOutput, SDKError>;
    /// Loads |model_id| and retrieves the input parameters.
    fn model_get_input_params(
        &mut self,
        app_id: SDKAppId,
        model_id: &str,
    ) -> Result<(ModelId, ModelInput), SDKError>;
    /// Set input data for the next run of model |id|.
    fn model_set_input(
        &mut self,
        app_id: SDKAppId,
        id: ModelId,
        input_data_offset: u32,
        input_data: &[u8],
    ) -> Result<(), SDKError>;

    /// Resets the audio framework.
    fn audio_reset(
        &mut self,
        app_id: SDKAppId,
        rxrst: bool, // Reset rx
        txrst: bool, // Reset tx
        rxilvl: u8,  // RX fifo level (one of 1,4,8,16,30)
        txilvl: u8,  // TX fifo level (one of 1,4,8,16)
    ) -> Result<(), SDKError>;
    /// Start recording audio into a buffer of size |buffer_size| using
    /// |rate| sampling. If the buffer fills before a stop request is
    /// received recording is automatically stopped.
    fn audio_record_start(
        &mut self,
        app_id: SDKAppId,
        rate: usize,
        buffer_size: usize,
        stop_on_full: bool,
    ) -> Result<(), SDKError>;
    /// Collects data from a recording started with |audio_record_start|.
    /// The data are returned in native (hardware) format.
    fn audio_record_collect(
        &mut self,
        app_id: SDKAppId,
        max_samples: usize,
        wait_if_empty: bool,
    ) -> Result<&[u32], SDKError>;
    /// Stop a recording session started with |audio_record_start|.
    fn audio_record_stop(&mut self, app_id: SDKAppId) -> Result<(), SDKError>;

    /// Start playing audio data with |rate| sampling.
    fn audio_play_start(
        &mut self,
        app_id: SDKAppId,
        rate: usize,
        buffer_size: usize,
    ) -> Result<(), SDKError>;
    /// Writes data according to |audio_play_start|.
    /// The data are assumed in native (hardware) format.
    fn audio_play_write(&mut self, app_id: SDKAppId, data: &[u32]) -> Result<(), SDKError>;
    /// Stop a play session started with |audio_play_start|.
    fn audio_play_stop(&mut self, app_id: SDKAppId) -> Result<(), SDKError>;
}

/// Rust client-side request processing. Note there is no CAmkES stub to
/// call; everything is done here. A single page frame is attached to the
/// IPC buffer with request parameters in the first half and return values
/// in the second half. Requests must have an SDKRequestHeader written to
/// the label field of the MessageInfo. Responses must have an SDKRuntimeError
/// written to the label field of the reply. For the moment this uses
/// postcard for serde work; this may change in the future (e.g. to flatbuffers).
///
/// The caller is responsible for synchronizing access to CANTRIP_SDK_* state
/// and the IPC buffer.
//
// TODO(sleffler): this attaches the call params to the IPC; might be
//   better to keep the page(s) mapped in SDKRuntime to avoid map/unmap
//   per-RPC but that requires a vspace allocator (or something special
//   purpose) and a redesign of the server side to use the endpoint badge
//   to lookup the mapped page early. Downside to a fixed mapping is it
//   limits how to handle requests w/ different-sized params (e.g. sensor
//   frame vs key-value params).
fn sdk_request<'a, S: Serialize, D: Deserialize<'a>>(
    request: SDKRuntimeRequest,
    request_args: &S,
) -> Result<D, SDKRuntimeError> {
    let params_slice = unsafe { core::slice::from_raw_parts_mut(CANTRIP_SDK_PARAMS, PAGE_SIZE) };

    // NB: server-side must do the same split
    let (request_slice, reply_slice) = params_slice.split_at_mut(SDKRUNTIME_REQUEST_DATA_SIZE);

    // Encode request arguments.
    let _ = postcard::to_slice(request_args, request_slice)
        .or(Err(SDKRuntimeError::SDKSerializeFailed))?;

    // Attach params & call the SDKRuntime; then wait (block) for a reply.
    unsafe {
        seL4_SetCap(0, CANTRIP_SDK_FRAME);
        let info = seL4_Call(
            CANTRIP_SDK_ENDPOINT,
            seL4_MessageInfo::new(
                /*label=*/ request.into(),
                /*capsUnrapped=*/ 0,
                /*extraCaps=*/ 1,
                /*length=*/ 0,
            ),
        );
        seL4_SetCap(0, 0);

        let status = SDKRuntimeError::try_from(info.get_label())
            .or(Err(SDKRuntimeError::SDKUnknownResponse))?;
        if status != SDKRuntimeError::SDKSuccess {
            return Err(status);
        }
    }

    // Decode response data.
    postcard::from_bytes::<D>(reply_slice).or(Err(SDKRuntimeError::SDKDeserializeFailed))
}

/// Rust client-side wrapper for the ping method.
#[inline]
pub fn sdk_ping() -> Result<(), SDKRuntimeError> {
    sdk_request::<PingRequest, ()>(SDKRuntimeRequest::Ping, &PingRequest {})
}

/// Rust client-side wrapper for the log method.
#[inline]
pub fn sdk_log(msg: &str) -> Result<(), SDKRuntimeError> {
    sdk_request::<LogRequest, ()>(
        SDKRuntimeRequest::Log,
        &LogRequest {
            msg: msg.as_bytes(),
        },
    )
}

/// Rust client-side wrapper for the read key method.
// TODO(sleffler): _mut variant?
#[inline]
pub fn sdk_read_key<'a>(key: &str, keyval: &'a mut [u8]) -> Result<&'a [u8], SDKRuntimeError> {
    let response = sdk_request::<ReadKeyRequest, ReadKeyResponse>(
        SDKRuntimeRequest::ReadKey,
        &ReadKeyRequest { key },
    )?;
    keyval.copy_from_slice(response.value);
    Ok(keyval)
}

/// Rust client-side wrapper for the write key method.
#[inline]
pub fn sdk_write_key(key: &str, value: &[u8]) -> Result<(), SDKRuntimeError> {
    sdk_request::<WriteKeyRequest, ()>(SDKRuntimeRequest::WriteKey, &WriteKeyRequest { key, value })
}

/// Rust client-side wrapper for the delete key method.
#[inline]
pub fn sdk_delete_key(key: &str) -> Result<(), SDKRuntimeError> {
    sdk_request::<DeleteKeyRequest, ()>(SDKRuntimeRequest::DeleteKey, &DeleteKeyRequest { key })
}

/// Rust client-side wrapper for the timer_oneshot method.
#[inline]
pub fn sdk_timer_oneshot(id: TimerId, duration_ms: TimerDuration) -> Result<(), SDKRuntimeError> {
    sdk_request::<TimerStartRequest, ()>(
        SDKRuntimeRequest::OneshotTimer,
        &TimerStartRequest { id, duration_ms },
    )
}

/// Rust client-side wrapper for the timer_periodic method.
#[inline]
pub fn sdk_timer_periodic(id: TimerId, duration_ms: TimerDuration) -> Result<(), SDKRuntimeError> {
    sdk_request::<TimerStartRequest, ()>(
        SDKRuntimeRequest::PeriodicTimer,
        &TimerStartRequest { id, duration_ms },
    )
}

/// Rust client-side wrapper for the timer_cancel method.
#[inline]
pub fn sdk_timer_cancel(id: TimerId) -> Result<(), SDKRuntimeError> {
    sdk_request::<TimerCancelRequest, ()>(
        SDKRuntimeRequest::CancelTimer,
        &TimerCancelRequest { id },
    )
}

/// Rust client-side wrapper for the timer_wait method.
#[inline]
pub fn sdk_timer_wait() -> Result<TimerMask, SDKRuntimeError> {
    let response = sdk_request::<TimerWaitRequest, TimerWaitResponse>(
        SDKRuntimeRequest::WaitForTimers,
        &TimerWaitRequest {},
    )?;
    Ok(response.mask)
}

/// Rust client-side wrapper for the timer_poll method.
#[inline]
pub fn sdk_timer_poll() -> Result<TimerMask, SDKRuntimeError> {
    let response = sdk_request::<TimerWaitRequest, TimerWaitResponse>(
        SDKRuntimeRequest::PollForTimers,
        &TimerWaitRequest {},
    )?;
    Ok(response.mask)
}

/// Rust client-side wrapper for the model_oneshot method.
#[inline]
pub fn sdk_model_oneshot(model_id: &str) -> Result<ModelId, SDKRuntimeError> {
    let response = sdk_request::<ModelOneshotRequest, ModelStartResponse>(
        SDKRuntimeRequest::OneshotModel,
        &ModelOneshotRequest { model_id },
    )?;
    Ok(response.id)
}

/// Rust client-side wrapper for the model_periodic method.
#[inline]
pub fn sdk_model_periodic(
    model_id: &str,
    duration_ms: TimerDuration,
) -> Result<ModelId, SDKRuntimeError> {
    let response = sdk_request::<ModelPeriodicRequest, ModelStartResponse>(
        SDKRuntimeRequest::PeriodicModel,
        &ModelPeriodicRequest {
            model_id,
            duration_ms,
        },
    )?;
    Ok(response.id)
}

/// Rust client-side wrapper for the model_cancel method.
#[inline]
pub fn sdk_model_cancel(id: ModelId) -> Result<(), SDKRuntimeError> {
    sdk_request::<ModelCancelRequest, ()>(
        SDKRuntimeRequest::CancelModel,
        &ModelCancelRequest { id },
    )
}

/// Rust client-side wrapper for the model_wait method.
#[inline]
pub fn sdk_model_wait() -> Result<ModelMask, SDKRuntimeError> {
    let response = sdk_request::<ModelWaitRequest, ModelWaitResponse>(
        SDKRuntimeRequest::WaitForModel,
        &ModelWaitRequest {},
    )?;
    Ok(response.mask)
}

/// Rust client-side wrapper for the model_poll method.
#[inline]
pub fn sdk_model_poll() -> Result<ModelMask, SDKRuntimeError> {
    let response = sdk_request::<ModelWaitRequest, ModelWaitResponse>(
        SDKRuntimeRequest::PollForModels,
        &ModelWaitRequest {},
    )?;
    Ok(response.mask)
}

/// Rust client-side wrapper for the model_output method.
#[inline]
pub fn sdk_model_output(id: ModelId) -> Result<ModelOutput, SDKRuntimeError> {
    let response = sdk_request::<ModelOutputRequest, ModelOutputResponse>(
        SDKRuntimeRequest::GetModelOutput,
        &ModelOutputRequest { id },
    )?;
    Ok(response.output)
}

/// Rust client-side wrapper for the model_get_input_params method.
#[inline]
pub fn sdk_model_get_input_params(
    model_id: &str,
) -> Result<(ModelId, ModelInput), SDKRuntimeError> {
    let response = sdk_request::<ModelGetInputParamsRequest, ModelGetInputParamsResponse>(
        SDKRuntimeRequest::GetModelInputParams,
        &ModelGetInputParamsRequest { model_id },
    )?;
    Ok((response.id, response.input_params))
}

/// Rust client-side wrapper for the model_set_input method.
#[inline]
pub fn sdk_model_set_input(
    id: ModelId,
    input_data_offset: u32,
    input_data: &[u8],
) -> Result<(), SDKRuntimeError> {
    sdk_request::<ModelSetInputRequest, ()>(
        SDKRuntimeRequest::SetModelInput,
        &ModelSetInputRequest {
            id,
            input_data_offset,
            input_data,
        },
    )
}

#[inline]
pub fn sdk_audio_reset(
    rxrst: bool,
    txrst: bool,
    rxilvl: u8,
    txilvl: u8,
) -> Result<(), SDKRuntimeError> {
    sdk_request::<AudioResetRequest, ()>(
        SDKRuntimeRequest::AudioReset,
        &AudioResetRequest {
            rxrst,
            txrst,
            rxilvl,
            txilvl,
        },
    )
}

#[inline]
pub fn sdk_audio_record_start(
    rate: usize,
    buffer_size: usize,
    stop_on_full: bool,
) -> Result<(), SDKRuntimeError> {
    sdk_request::<AudioRecordStartRequest, ()>(
        SDKRuntimeRequest::AudioRecordStart,
        &AudioRecordStartRequest {
            rate,
            buffer_size,
            stop_on_full,
        },
    )
}

#[inline]
pub fn sdk_audio_record_collect_non_blocking(data: &mut [u32]) -> Result<usize, SDKRuntimeError> {
    let response = sdk_request::<AudioRecordCollectRequest, AudioRecordCollectResponse>(
        SDKRuntimeRequest::AudioRecordCollect,
        &AudioRecordCollectRequest {
            max_samples: data.len(),
            wait_if_empty: false,
        },
    )?;
    data[..response.data.len()].copy_from_slice(response.data.to_vec().as_slice());
    Ok(response.data.len())
}

#[inline]
pub fn sdk_audio_record_collect(data: &mut [u32]) -> Result<usize, SDKRuntimeError> {
    let response = sdk_request::<AudioRecordCollectRequest, AudioRecordCollectResponse>(
        SDKRuntimeRequest::AudioRecordCollect,
        &AudioRecordCollectRequest {
            max_samples: data.len(),
            wait_if_empty: true,
        },
    )?;
    data[..response.data.len()].copy_from_slice(response.data.to_vec().as_slice());
    Ok(response.data.len())
}

#[inline]
pub fn sdk_audio_record_stop() -> Result<(), SDKRuntimeError> {
    sdk_request::<AudioRecordStopRequest, ()>(
        SDKRuntimeRequest::AudioRecordStop,
        &AudioRecordStopRequest {},
    )
}

#[inline]
pub fn sdk_audio_play_start(rate: usize, buffer_size: usize) -> Result<(), SDKRuntimeError> {
    sdk_request::<AudioPlayStartRequest, ()>(
        SDKRuntimeRequest::AudioPlayStart,
        &AudioPlayStartRequest { rate, buffer_size },
    )
}

#[inline]
pub fn sdk_audio_play_write(data: &[u32]) -> Result<(), SDKRuntimeError> {
    sdk_request::<AudioPlayWriteRequest, ()>(
        SDKRuntimeRequest::AudioPlayWrite,
        &AudioPlayWriteRequest {
            data: ZeroVec::from_slice_or_alloc(data),
        },
    )
}

#[inline]
pub fn sdk_audio_play_stop() -> Result<(), SDKRuntimeError> {
    sdk_request::<AudioPlayStopRequest, ()>(
        SDKRuntimeRequest::AudioPlayStop,
        &AudioPlayStopRequest {},
    )
}
