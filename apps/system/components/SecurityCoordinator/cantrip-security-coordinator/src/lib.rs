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

//! Cantrip OS security coordinator support

#![cfg_attr(not(test), no_std)]
#![allow(stable_features)]
// NB: "error[E0658]: trait bounds other than `Sized` on const fn parameters are unstable"
#![feature(const_fn_trait_bound)]

extern crate alloc;
use crate::upload::Upload;
use alloc::string::{String, ToString};
use cantrip_memory_interface::cantrip_cnode_alloc;
use cantrip_memory_interface::cantrip_object_free_in_cnode;
use cantrip_memory_interface::ObjDescBundle;
use cantrip_os_common::copyregion::CopyRegion;
use cantrip_os_common::cspace_slot::CSpaceSlot;
use cantrip_os_common::sel4_sys;
use cantrip_security_interface::*;
use hashbrown::HashMap;

use sel4_sys::seL4_Error;

#[cfg(all(feature = "fake", feature = "sec"))]
compile_error!("features \"fake\" and \"sec\" are mutually exclusive");

#[cfg_attr(feature = "sec", path = "sec/mod.rs")]
#[cfg_attr(feature = "fake", path = "fake/mod.rs")]
mod manager;
pub use manager::CantripSecurityManager;

mod upload;

pub const CAPACITY_BUNDLES: usize = 10; // HashMap of bundles

const APP_SUFFIX: &str = ".app";
const MODEL_SUFFIX: &str = ".model";
const KELVIN_SUFFIX: &str = ".kelvin";

extern "Rust" {
    // Regions for deep_copy work.
    fn get_deep_copy_src_mut() -> &'static mut [u8];
    fn get_deep_copy_dest_mut() -> &'static mut [u8];
}

/// Package contents either come from built-in files or dynamically
/// loaded from the DebugConsole. Builtin package data resides in (possibly simulated)
/// Flash. Dynamically loaded package data are stored in memory obtained from
/// the MemoryManager.
enum PkgContents {
    Flash(&'static [u8]), // Data resides in flash
    #[allow(dead_code)]
    Dynamic(ObjDescBundle), // Data resides in dynamically allocated memory
}

pub struct BundleData {
    pkg_contents: PkgContents,
    pkg_size: usize,
}
impl BundleData {
    // Returns a bundle for a dynamically loaded package.
    #[allow(dead_code)]
    fn new(pkg_contents: &ObjDescBundle) -> Self {
        Self {
            pkg_contents: PkgContents::Dynamic(pkg_contents.clone()),
            pkg_size: pkg_contents.size_bytes(),
        }
    }

    // Returns a bundle for a builtin package.
    fn new_from_flash(slice: &'static [u8]) -> Self {
        Self {
            pkg_contents: PkgContents::Flash(slice),
            pkg_size: slice.len(),
        }
    }

    // Returns a copy of the package contents suitable for sending
    // to another thread. The data are copied to newly allocated frames
    // and the frames are aggregated in a CNode ready to attach to
    // an IPC message.
    fn deep_copy(&self) -> Result<ObjDescBundle, seL4_Error> {
        let mut upload = match &self.pkg_contents {
            PkgContents::Flash(data) => upload_slice(data),
            PkgContents::Dynamic(bundle) => upload_obj_bundle(bundle),
        }?;

        // Collect the frames in a top-level CNode.
        let cnode_depth = upload.frames().count_log2();
        let cnode =
            cantrip_cnode_alloc(cnode_depth).map_err(|_| seL4_Error::seL4_NotEnoughMemory)?; // TODO(sleffler) From mapping
        upload
            .frames_mut()
            .move_objects_from_toplevel(cnode.objs[0].cptr, cnode_depth as u8)?;
        Ok(upload.frames().clone())
    }
}
impl Drop for BundleData {
    fn drop(&mut self) {
        if let PkgContents::Dynamic(bundle) = &self.pkg_contents {
            let _ = cantrip_object_free_in_cnode(bundle);
        }
    }
}

// Interface to back-end implementation.
pub trait SecurityManagerInterface {
    fn get_builtins(&self) -> BundleIdArray;
    fn lookup_builtin(&self, filename: &str) -> Option<&'static [u8]>;
    fn uninstall(&mut self, bundle_id: &str) -> Result<(), SecurityRequestError>;
    fn load_application(
        &mut self,
        bundle_id: &str,
        bundle_data: &BundleData,
    ) -> Result<ObjDescBundle, SecurityRequestError>;
    fn load_model(&self, model_data: &BundleData) -> Result<ObjDescBundle, SecurityRequestError>;
    fn read_key(&self, bundle_id: &str, key: &str) -> Result<&KeyValueData, SecurityRequestError>;
    fn write_key(
        &mut self,
        bundle_id: &str,
        key: &str,
        value: &[u8],
    ) -> Result<(), SecurityRequestError>;
    fn delete_key(&mut self, bundle_id: &str, key: &str) -> Result<(), SecurityRequestError>;
    fn test(&self) -> Result<(), SecurityRequestError>;
}

// Returns a copy (including seL4 objects) of |src| in an Upload container.
fn upload_obj_bundle(src: &ObjDescBundle) -> Result<Upload, seL4_Error> {
    // Dest is an upload object that allocates a page at-a-time so
    // the MemoryManager doesn't have to handle a huge memory request.
    let mut dest = Upload::new(unsafe { get_deep_copy_dest_mut() });

    // Src top-level slot & copy region
    let src_slot = CSpaceSlot::new();
    let mut src_region = unsafe { CopyRegion::new(get_deep_copy_src_mut()) };

    for src_cptr in src.cptr_iter() {
        // Map src frame and copy data (allocating memory as needed)..
        src_slot
            .dup_to(src.cnode, src_cptr, src.depth)
            .and_then(|_| src_region.map(src_slot.slot))?;
        dest.write(src_region.as_ref())
            .or(Err(seL4_Error::seL4_NotEnoughMemory))?; // TODO(sleffler) From mapping

        // Unmap & clear top-level src slot required for mapping.
        src_region.unmap().and_then(|_| src_slot.delete())?;
    }
    dest.finish();
    Ok(dest)
}

// Returns a copy (including seL4 objects) of |src| in an Upload container.
fn upload_slice(src: &[u8]) -> Result<Upload, seL4_Error> {
    // Dest is an upload object that allocates a page at-a-time so
    // the MemoryManager doesn't have to handle a huge memory request.
    let mut dest = Upload::new(unsafe { get_deep_copy_dest_mut() });
    dest.write(src).or(Err(seL4_Error::seL4_NotEnoughMemory))?;
    dest.finish();
    Ok(dest)
}

// Returns |key| or |key|+|suffix| if |key| does not end with |suffix|.
fn promote_key(key: &str, suffixes: &[&str]) -> String {
    for suf in suffixes {
        if key.ends_with(suf) {
            return key.to_string();
        }
    }
    key.to_string() + suffixes[0]
}

// CantripSecurityCoordinator bundles an instance of the SecurityCoordinator that operates
// on CantripOS interfaces. There is a two-step dance to setup an instance because we want
// CANTRIP_SECURITY static.
// NB: no locking is done; we assume the caller/user is single-threaded
pub struct CantripSecurityCoordinator {
    manager: CantripSecurityManager,
    bundles: HashMap<String, BundleData>,
}
impl Default for CantripSecurityCoordinator {
    fn default() -> Self { Self::new() }
}
impl CantripSecurityCoordinator {
    // Constructs a partially-initialized instance; to complete call init().
    // This is needed because we need a const fn for static setup.
    pub fn new() -> CantripSecurityCoordinator {
        Self {
            manager: CantripSecurityManager::new(),
            bundles: HashMap::with_capacity(CAPACITY_BUNDLES),
        }
    }

    // Probes for a bundle named |key| or |key|+<suffix>; returning Some(v)
    // where |v| is the key under which the bundle is registered.
    fn find_key(&self, key: &str) -> Result<String, SecurityRequestError> {
        if self.bundles.contains_key(key) {
            Ok(key.to_string())
        } else if self.bundles.contains_key(&(key.to_string() + APP_SUFFIX)) {
            Ok(key.to_string() + APP_SUFFIX)
        } else if self
            .bundles
            .contains_key(&(key.to_string() + KELVIN_SUFFIX))
        {
            Ok(key.to_string() + KELVIN_SUFFIX)
        } else if self.bundles.contains_key(&(key.to_string() + MODEL_SUFFIX)) {
            Ok(key.to_string() + MODEL_SUFFIX)
        } else {
            Err(SecurityRequestError::BundleNotFound)
        }
    }

    // Returns a ref for |bundle_id|'s entry.
    fn get_bundle(&self, bundle_id: &str) -> Result<&BundleData, SecurityRequestError> {
        self.find_key(bundle_id)
            .map(|key| self.bundles.get(&key).unwrap())
    }

    // Returns a bundle backed by builtin data.
    fn get_bundle_from_builtins(&self, filename: &str) -> Result<BundleData, SecurityRequestError> {
        self.manager
            .lookup_builtin(filename)
            .ok_or(SecurityRequestError::BundleNotFound)
            .map(BundleData::new_from_flash)
    }

    // Remove any entry for |bundle_id|.
    fn remove_bundle(&mut self, bundle_id: &str) -> Result<(), SecurityRequestError> {
        self.find_key(bundle_id)
            .map(|key| self.bundles.remove(&key))
            .map(|_| ())
    }
}

impl SecurityCoordinatorInterface for CantripSecurityCoordinator {
    fn install(&mut self, _pkg_contents: &ObjDescBundle) -> Result<String, SecurityRequestError> {
        // Replaced by install_app & install_model
        Err(SecurityRequestError::InstallFailed)
    }
    fn install_app(
        &mut self,
        app_id: &str,
        _pkg_contents: &ObjDescBundle,
    ) -> Result<(), SecurityRequestError> {
        let key = promote_key(app_id, &[APP_SUFFIX]);
        if self.bundles.contains_key(&key) {
            return Err(SecurityRequestError::DeleteFirst);
        }
        // XXX defer to back-end impl.
        Err(SecurityRequestError::InstallFailed)
    }
    fn install_model(
        &mut self,
        _app_id: &str,
        model_id: &str,
        _pkg_contents: &ObjDescBundle,
    ) -> Result<(), SecurityRequestError> {
        // NB: no key promotion, model name must be fully specified
        let key = promote_key(model_id, &[""]);
        if self.bundles.contains_key(&key) {
            return Err(SecurityRequestError::DeleteFirst);
        }
        // XXX defer to back-end impl.
        Err(SecurityRequestError::InstallFailed)
    }
    fn uninstall(&mut self, bundle_id: &str) -> Result<(), SecurityRequestError> {
        // NB: does not remove flash/built-in contents
        let _ = self.manager.uninstall(bundle_id);
        self.remove_bundle(bundle_id)
    }

    fn get_packages(&self) -> Result<BundleIdArray, SecurityRequestError> {
        // First, dynamically installed bundles.
        let mut result: BundleIdArray = self.bundles.keys().cloned().collect();
        // Second, builtins.
        result.append(&mut self.manager.get_builtins());
        result.sort();
        result.dedup();
        Ok(result)
    }

    // TODO(sleffler): use get_bundle so package must be loaded? instantiating
    //   hashmap entries may be undesirable
    fn size_buffer(&self, bundle_id: &str) -> Result<usize, SecurityRequestError> {
        let bundle = self.get_bundle(bundle_id)?;
        Ok(bundle.pkg_size) // TODO(sleffler): do better
    }
    fn get_manifest(&self, bundle_id: &str) -> Result<String, SecurityRequestError> {
        let _bundle = self.get_bundle(bundle_id)?;
        Err(SecurityRequestError::GetManifestFailed)
    }

    fn load_application(&mut self, bundle_id: &str) -> Result<ObjDescBundle, SecurityRequestError> {
        // NB: loading may promote a bundle from the built-ins archive to the hashmap
        if self.bundles.contains_key(bundle_id) {
            return self
                .manager
                .load_application(bundle_id, self.bundles.get(bundle_id).unwrap());
        }
        if let Ok(bd) = self.get_bundle_from_builtins(bundle_id) {
            assert!(self.bundles.insert(bundle_id.to_string(), bd).is_none());
            return self
                .manager
                .load_application(bundle_id, self.bundles.get(bundle_id).unwrap());
        }
        let key = promote_key(bundle_id, &[APP_SUFFIX]);
        if !self.bundles.contains_key(&key) {
            let bd = self.get_bundle_from_builtins(&key)?;
            assert!(self.bundles.insert(key.clone(), bd).is_none());
        }
        self.manager
            .load_application(&key, self.bundles.get(&key).unwrap())
    }
    fn load_model(
        &mut self,
        _bundle_id: &str, // TODO(sleffler): models are meant to be associated with bundle_id
        model_id: &str,
    ) -> Result<ObjDescBundle, SecurityRequestError> {
        fn load_model_data(model_data: &BundleData) -> Result<ObjDescBundle, SecurityRequestError> {
            // Clone everything (struct + associated seL4 objects) so the
            // return is as though it was newly instantiated from flash.
            model_data
                .deep_copy()
                .or(Err(SecurityRequestError::LoadModelFailed))
        }
        if let Some(bd) = self.bundles.get(model_id) {
            return load_model_data(bd);
        }
        if let Ok(bd) = self.get_bundle_from_builtins(model_id) {
            // No need to add to bundles since no other calls make sense
            // (though perhaps size_buffer might be useful).
            return load_model_data(&bd);
        }
        // NB: no key promotion, model name must be fully specified
        Err(SecurityRequestError::BundleNotFound)
    }

    // NB: key-value ops require a load'd bundle so only do get_bundle
    fn read_key(&self, bundle_id: &str, key: &str) -> Result<&KeyValueData, SecurityRequestError> {
        self.manager.read_key(&self.find_key(bundle_id)?, key)
    }
    fn write_key(
        &mut self,
        bundle_id: &str,
        key: &str,
        value: &[u8],
    ) -> Result<(), SecurityRequestError> {
        self.manager
            .write_key(&self.find_key(bundle_id)?, key, value)
    }
    fn delete_key(&mut self, bundle_id: &str, key: &str) -> Result<(), SecurityRequestError> {
        self.manager.delete_key(&self.find_key(bundle_id)?, key)
    }

    fn test(&mut self) -> Result<(), SecurityRequestError> { self.manager.test() }
}
