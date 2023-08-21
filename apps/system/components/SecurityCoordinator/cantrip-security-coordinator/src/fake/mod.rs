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

//! Cantrip OS security coordinator fake manager

use crate::BundleData;
use crate::SecurityManagerInterface;
use alloc::string::{String, ToString};
use cantrip_security_interface::*;
use cpio::CpioNewcReader;
use hashbrown::HashMap;
use log::error;

const CAPACITY_KEYS: usize = 2; // Per-bundle HashMap of key-values

extern "Rust" {
    fn get_cpio_archive() -> &'static [u8]; // CPIO archive of built-in files
}

struct FakeBundleData {
    keys: HashMap<String, KeyValueData>,
}
impl FakeBundleData {
    fn new() -> Self {
        Self {
            keys: HashMap::with_capacity(CAPACITY_KEYS),
        }
    }
}
pub struct FakeSecurityManager {
    bundles: HashMap<String, FakeBundleData>,
}
impl Default for FakeSecurityManager {
    fn default() -> Self { Self::new() }
}
pub type CantripSecurityManager = FakeSecurityManager; // Bind public name/type

impl FakeSecurityManager {
    pub fn new() -> Self {
        Self {
            bundles: HashMap::with_capacity(crate::CAPACITY_BUNDLES),
        }
    }

    // Returns a ref for |bundle_id|'s entry.
    fn get_bundle(&self, bundle_id: &str) -> Result<&FakeBundleData, SecurityRequestError> {
        self.bundles
            .get(bundle_id)
            .ok_or(SecurityRequestError::BundleNotFound)
    }
    // Returns a mutable ref for |bundle_id|'s entry.
    fn get_bundle_mut(
        &mut self,
        bundle_id: &str,
    ) -> Result<&mut FakeBundleData, SecurityRequestError> {
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

impl SecurityManagerInterface for FakeSecurityManager {
    // Returns an array of bundle id's from the builtin archive.
    fn get_builtins(&self) -> Result<BundleIdArray, SecurityRequestError> {
        let mut builtins = BundleIdArray::new();
        for e in CpioNewcReader::new(unsafe { get_cpio_archive() }) {
            match e {
                Err(err) => {
                    error!("cpio read err {:?}", err);
                    return Err(SecurityRequestError::GetPackagesFailed);
                }
                Ok(entry) => builtins.push(entry.name.to_string()),
            }
        }
        Ok(builtins)
    }

    // Returns a bundle backed by builtin data.
    fn lookup_builtin(&self, filename: &str) -> Result<BundleData, SecurityRequestError> {
        for e in CpioNewcReader::new(unsafe { get_cpio_archive() }) {
            match e {
                Err(err) => {
                    error!("cpio read err {:?}", err);
                    return Err(SecurityRequestError::BundleNotFound);
                }
                Ok(entry) => {
                    if entry.name == filename {
                        return Ok(BundleData::new_from_flash(entry.data));
                    }
                }
            }
        }
        Err(SecurityRequestError::BundleNotFound)
    }

    fn uninstall(&mut self, bundle_id: &str) -> Result<(), SecurityRequestError> {
        self.remove_bundle(bundle_id)
    }

    fn load_application(&mut self, bundle_id: &str) -> Result<(), SecurityRequestError> {
        // Create an local entry for possible key ops. Note this persists
        // until the app is uninstall'd. If an app is loaded multiple
        // times w/o an uninstall this will replace any existing with a
        // new/empty hashmap.
        self.bundles
            .insert(bundle_id.to_string(), FakeBundleData::new());
        Ok(())
    }

    // NB: key-value ops require a load'd application so only do get_bundle
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
    fn test(&self, _count: usize) -> Result<(), SecurityRequestError> {
        Err(SecurityRequestError::TestFailed)
    }
}
