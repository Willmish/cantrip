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

//! Cantrip OS security coordinator Security Core (SEC) manager

use crate::BundleData;
use crate::SecurityManagerInterface;
use alloc::string::{String, ToString};
use cantrip_memory_interface::cantrip_frame_alloc;
use cantrip_memory_interface::cantrip_object_free_toplevel;
use cantrip_memory_interface::ObjDescBundle;
use cantrip_os_common::copyregion::CopyRegion;
use cantrip_os_common::sel4_sys;
use cantrip_security_interface::*;
use core::mem::size_of;
use hashbrown::HashMap;
use log::trace;
use mailbox_interface::*;

use sel4_sys::seL4_PageBits;
use sel4_sys::seL4_Page_GetAddress;

const CAPACITY_KEYS: usize = 2; // Per-bundle HashMap of key-values

extern "Rust" {
    fn get_deep_copy_src_mut() -> &'static mut [u8];
}

struct SecBundleData {
    keys: HashMap<String, KeyValueData>, // NB: emulate until SEC has support
}
impl SecBundleData {
    fn new() -> Self {
        Self {
            keys: HashMap::with_capacity(CAPACITY_KEYS),
        }
    }
}
pub struct SecSecurityManager {
    bundles: HashMap<String, SecBundleData>,
    // TODO(sleffler): mailbox api state?
}
impl Default for SecSecurityManager {
    fn default() -> Self { Self::new() }
}
impl SecSecurityManager {
    pub fn new() -> Self {
        Self {
            bundles: HashMap::with_capacity(crate::CAPACITY_BUNDLES),
        }
    }

    // Returns a ref for |bundle_id|'s entry.
    fn get_bundle(&self, bundle_id: &str) -> Result<&SecBundleData, SecurityRequestError> {
        self.bundles
            .get(bundle_id)
            .ok_or(SecurityRequestError::BundleNotFound)
    }
    // Returns a mutable ref for |bundle_id|'s entry.
    fn get_bundle_mut(
        &mut self,
        bundle_id: &str,
    ) -> Result<&mut SecBundleData, SecurityRequestError> {
        self.bundles
            .get_mut(bundle_id)
            .ok_or(SecurityRequestError::BundleNotFound)
    }

    // Remove any entry for |bundle_id|.
    fn remove_bundle(&mut self, bundle_id: &str) -> Result<(), SecurityRequestError> {
        self.bundles
            .remove(bundle_id)
            .and(Some(()))
            .ok_or(SecurityRequestError::BundleNotFound)
    }
}
pub type CantripSecurityManager = SecSecurityManager; // Bind public name/type

impl SecurityManagerInterface for SecSecurityManager {
    // Returns an array of bundle id's from the builtin archive.
    fn get_builtins(&self) -> BundleIdArray {
        // XXX fill-in
        BundleIdArray::new()
    }

    // Returns a bundle backed by builtin data.
    fn lookup_builtin(&self, _filename: &str) -> Option<&'static [u8]> {
        // XXX fill-in
        None
    }

    fn uninstall(&mut self, bundle_id: &str) -> Result<(), SecurityRequestError> {
        self.remove_bundle(bundle_id)
    }

    fn load_application(
        &mut self,
        bundle_id: &str,
        bundle_data: &BundleData,
    ) -> Result<ObjDescBundle, SecurityRequestError> {
        // Clone everything (struct + associated seL4 objects) so the
        // return is as though it was newly instantiated from flash.
        // XXX just return the package for now
        let app_bundle = bundle_data
            .deep_copy()
            .or(Err(SecurityRequestError::LoadApplicationFailed))?;

        // Create an sec bundle for possible key ops. Note this persists
        // until the app is uninstall'd. If an app is loaded multiple
        // times w/o an uninstall this will replace any existing with a
        // new/empty hashmap.
        self.bundles
            .insert(bundle_id.to_string(), SecBundleData::new());

        Ok(app_bundle)
    }

    fn load_model(&self, model_data: &BundleData) -> Result<ObjDescBundle, SecurityRequestError> {
        // Clone everything (struct + associated seL4 objects) so the
        // return is as though it was newly instantiated from flash.
        model_data
            .deep_copy()
            .or(Err(SecurityRequestError::LoadModelFailed))
    }

    // NB: key-value ops require a load'd bundle so only do get_bundle
    fn read_key(&self, bundle_id: &str, key: &str) -> Result<&KeyValueData, SecurityRequestError> {
        let bundle = self.get_bundle(bundle_id)?;
        bundle
            .keys
            .get(key)
            .ok_or(SecurityRequestError::KeyNotFound)
    }
    fn write_key(
        &mut self,
        bundle_id: &str,
        key: &str,
        value: &[u8],
    ) -> Result<(), SecurityRequestError> {
        let bundle = self.get_bundle_mut(bundle_id)?;
        let mut keyval = [0u8; KEY_VALUE_DATA_SIZE];
        keyval[..value.len()].copy_from_slice(value);
        let _ = bundle.keys.insert(key.to_string(), keyval);
        Ok(())
    }
    fn delete_key(&mut self, bundle_id: &str, key: &str) -> Result<(), SecurityRequestError> {
        let bundle = self.get_bundle_mut(bundle_id)?;
        // TODO(sleffler): error if no entry?
        let _ = bundle.keys.remove(key);
        Ok(())
    }

    fn test(&self) -> Result<(), SecurityRequestError> {
        trace!("test manager begin");

        const MESSAGE_SIZE_DWORDS: usize = 17; // Just a random message size for testing.

        // Allocate a 4k page to serve as our message buffer.
        const PAGE_SIZE: usize = 1 << seL4_PageBits;
        let frame_bundle =
            cantrip_frame_alloc(PAGE_SIZE).or(Err(SecurityRequestError::TestFailed))?;
        trace!("test_mailbox: Frame {:?}", frame_bundle);

        unsafe {
            // Map the message buffer into our copyregion so we can access it.
            // NB: re-use one of the deep_copy copyregions.
            let mut msg_region = CopyRegion::new(get_deep_copy_src_mut());
            msg_region
                .map(frame_bundle.objs[0].cptr)
                .or(Err(SecurityRequestError::TestFailed))?;

            let message_ptr = msg_region.as_word_mut();

            // Write to the message buffer through the copyregion.
            let offset_a = 0;
            let offset_b = MESSAGE_SIZE_DWORDS - 1;
            message_ptr[offset_a] = 0xDEADBEEF;
            message_ptr[offset_b] = 0xF00DCAFE;
            trace!(
                "test_mailbox: old buf contents  0x{:X} 0x{:X}",
                message_ptr[offset_a],
                message_ptr[offset_b]
            );

            // Send the _physical_ address of the message buffer to the security
            // core.
            let paddr = seL4_Page_GetAddress(frame_bundle.objs[0].cptr);
            mailbox_send(paddr.paddr as u32, (MESSAGE_SIZE_DWORDS * size_of::<u32>()) as u32)
                .or(Err(SecurityRequestError::TestFailed))?;

            // Wait for the response to arrive.
            let _ = mailbox_recv().or(Err(SecurityRequestError::TestFailed))?;

            // The security core should have replaced the first and last dwords
            // with 0x12345678 and 0x87654321.
            trace!("test_mailbox: expected contents 0x12345678 0x87654321");
            trace!(
                "test_mailbox: new buf contents  0x{:X} 0x{:X}",
                message_ptr[offset_a],
                message_ptr[offset_b]
            );

            let dword_a = message_ptr[offset_a];
            let dword_b = message_ptr[offset_b];

            msg_region
                .unmap()
                .or(Err(SecurityRequestError::TestFailed))?;

            // Done, free the message buffer.
            cantrip_object_free_toplevel(&frame_bundle)
                .or(Err(SecurityRequestError::TestFailed))?;

            if dword_a != 0x12345678 || dword_b != 0x87654321 {
                return Err(SecurityRequestError::TestFailed);
            }
        }

        trace!("test manager done");
        Ok(())
    }
}
