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
use cantrip_security_interface::*;
use core::mem::size_of;
use hashbrown::HashMap;
use log::info;
use mailbox_driver::*;

const CAPACITY_KEYS: usize = 2; // Per-bundle HashMap of key-values

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
    fn get_builtins(&self) -> Result<BundleIdArray, SecurityRequestError> {
        mbox_get_builtins().or(Err(SecurityRequestError::GetPackagesFailed))
    }

    // Returns a bundle backed by builtin data.
    fn lookup_builtin(&self, filename: &str) -> Result<BundleData, SecurityRequestError> {
        mbox_find_file(filename)
            .or(Err(SecurityRequestError::BundleNotFound)) // XXX
            .map(|(fid, size_bytes)| BundleData::new_from_sec(fid, size_bytes as usize))
    }

    fn uninstall(&mut self, bundle_id: &str) -> Result<(), SecurityRequestError> {
        self.remove_bundle(bundle_id)
    }

    fn load_application(&mut self, bundle_id: &str) -> Result<(), SecurityRequestError> {
        // Create an sec bundle for possible key ops. Note this persists
        // until the app is uninstall'd. If an app is loaded multiple
        // times w/o an uninstall this will replace any existing with a
        // new/empty hashmap.
        self.bundles
            .insert(bundle_id.to_string(), SecBundleData::new());
        Ok(())
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

    fn test(&self, count: usize) -> Result<(), SecurityRequestError> {
        const MAX_WORDS: usize = 4096 / size_of::<u32>();
        if !(1 < count && count <= MAX_WORDS) {
            info!("Invalid word count {count}, must be in the range [2..{MAX_WORDS}]");
            return Err(SecurityRequestError::TestFailed);
        }

        fn test_mailbox(
            count: usize,
            frame_bundle: &ObjDescBundle,
        ) -> Result<(), SecurityRequestError> {
            // Map the message buffer using an existing copyregion.
            extern "Rust" {
                fn get_deep_copy_src_mut() -> &'static mut [u8];
            }
            let mut msg_region = unsafe { CopyRegion::new(get_deep_copy_src_mut()) };
            msg_region.map(frame_bundle.objs[0].cptr).expect("map");

            let msg = msg_region.as_word_mut();

            // Write initial values; we expect the SEC to overwrite.
            let first = 0;
            let last = count - 1;
            msg[first] = 0xDEADBEEF;
            msg[last] = 0xF00DCAFE;

            let sent_bytes = (count * size_of::<u32>()) as u32;
            let recv_bytes =
                mbox_test(frame_bundle.objs[0].cptr, sent_bytes).expect("mailbox_test");
            if recv_bytes != sent_bytes {
                info!("sent bytes {} != recv bytes {}", sent_bytes, recv_bytes);
            }

            // The security core should have replaced the first and last dwords
            // in msg with 0x12345678 and 0x87654321.
            if msg[first] != 0x12345678 || msg[last] != 0x87654321 {
                info!("initial data:  0xdeadbeef 0xf00dcafe");
                info!("expected data: 0x12345678 0x87654321");
                info!("received data: {:#08x} {:#08x}", msg[first], msg[last]);
                Err(SecurityRequestError::TestFailed)
            } else {
                Ok(())
            }
            // NB: msg_region unmapped on drop
        }

        // Allocate a 4k page to serve as our message buffer.
        let frame_bundle =
            cantrip_frame_alloc(4096).or(Err(SecurityRequestError::CapAllocFailed))?;
        let result = test_mailbox(count, &frame_bundle);
        let _ = cantrip_object_free_toplevel(&frame_bundle);

        result
    }
}
