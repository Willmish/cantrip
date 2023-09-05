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

// Fill file data from the cpio archive baked into capdl-loader.

use crate::CantripOsModel;
use capdl::*;
use core::ptr;
use cpio::CpioNewcReader;
use cstr_core::CStr;
use log::trace;

use sel4_sys::seL4_CPtr;
use sel4_sys::seL4_Result;

use static_assertions::assert_cfg;
assert_cfg!(feature = "CONFIG_CAPDL_LOADER_FILL_FROM_CPIO");

impl<'a> CantripOsModel<'a> {
    pub fn fill_begin(&mut self) {}
    pub fn fill_end(&mut self) {}

    // Fill a frame's contents from a file in the cpio archive;
    // in particular this loads each CAmkES component's executable.
    pub fn fill_frame_with_filedata(
        &mut self,
        sel4_frame: seL4_CPtr,
        frame_fill: &CDL_FrameFill_Element_t,
    ) -> seL4_Result {
        let cpio_lookup = |filename: &str| -> &[u8] {
            for e in CpioNewcReader::new(self.capdl_archive) {
                let entry = e.unwrap();
                if entry.name == filename {
                    return entry.data;
                }
            }
            panic!("{} not found in cpio archive", filename);
        };
        let file_data = frame_fill.get_file_data();
        let filename = unsafe { CStr::from_ptr(file_data.filename) }
            .to_str()
            .unwrap();
        // Check the last lookup before scanning the cpio archive.
        if filename != self.last_filename {
            trace!("switch filedata fill to {}", filename);
            self.last_data = cpio_lookup(filename);
            self.last_filename = filename;
        }
        let base = Self::map_copy_region(sel4_frame)?;
        unsafe {
            ptr::copy_nonoverlapping(
                ptr::addr_of!(self.last_data[file_data.file_offset]),
                (base + frame_fill.dest_offset) as *mut u8,
                frame_fill.dest_len,
            )
        }
        Self::unmap_copy_region(sel4_frame)
    }
}
