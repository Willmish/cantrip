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

// Fill file data from the Security Core.
// Uses a stripped-down synchronous mailbox driver.

use super::PAGE_SIZE;
use crate::CantripOsModel;
use capdl::*;
use cstr_core::CStr;
use log::trace;

use sel4_sys::seL4_Result;
use sel4_sys::*;

use mailbox_driver::*;

use static_assertions::assert_cfg;
assert_cfg!(feature = "CONFIG_CAPDL_LOADER_FILL_FROM_SEC");

// Glue for mailbox hw access (mimics camkes-generated bits).

#[repr(C, align(4096))]
struct mailbox_mmio {
    data: [u8; PAGE_SIZE],
}
static mut MAILBOX_MMIO: mailbox_mmio = mailbox_mmio {
    data: [0u8; PAGE_SIZE],
};
#[no_mangle]
pub fn get_mailbox_mmio() -> &'static [u8] { unsafe { &MAILBOX_MMIO.data[..] } }
#[no_mangle]
pub fn get_mailbox_mmio_mut() -> &'static mut [u8] { unsafe { &mut MAILBOX_MMIO.data[..] } }

impl<'a> CantripOsModel<'a> {
    // Sets up the mailbox driver to talk to the SecurityCoordinator.
    pub fn fill_begin(&mut self) {
        // When creating objects the page frame used to access the
        // mailbox hardware registers is stashed in self.mbox_frame.
        // We use that to map the mailbox csr's for use below.
        trace!("fill_begin mbox_frame {}", self.mbox_frame);
        assert!(is_objid_valid(self.mbox_frame));
        unsafe {
            // Unmap the memory region where we'll map the csr's.
            seL4_Page_Unmap(self.get_vaddr_cptr(get_mailbox_mmio().as_ptr() as usize))
                .expect("page_unmap");
        }
        Self::mbox_map(self.get_orig_cap(self.mbox_frame)).expect("mbox_map");
    }

    // Cleans up the work done by fill_begin.
    pub fn fill_end(&mut self) {
        trace!("fill_end mbox_frame {}", self.mbox_frame);
        Self::mbox_unmap(self.get_orig_cap(self.mbox_frame)).expect("mbox_unmap");
        // NB: leave mailbox_mmio unmapped
    }

    // Fill a frame's contents from a file in the Security Core;
    // in particular this loads each CAmkES component's executable.
    pub fn fill_frame_with_filedata(
        &mut self,
        sel4_frame: seL4_CPtr,
        frame_fill: &CDL_FrameFill_Element_t,
    ) -> seL4_Result {
        let file_data = frame_fill.get_file_data();
        let filename = unsafe { CStr::from_ptr(file_data.filename) }
            .to_str()
            .unwrap();
        // Check the last lookup before searching for the file.
        if filename != self.last_filename {
            trace!("switch filedata fill to {}", filename);
            (self.last_fid, _) = mbox_find_file(filename).or(Err(seL4_FailedLookup))?;
            self.last_filename = filename;
        }
        // XXX Could use a bounce page or copy if frame_fill.dest_offset != 0
        assert!(frame_fill.dest_offset == 0);
        assert!(frame_fill.dest_len <= PAGE_SIZE);
        assert!((file_data.file_offset % PAGE_SIZE) == 0);
        // TODO(sleffler): add offset + length to rpc (SEC fills entire page) & remove local hacks
        let base = Self::map_copy_region(sel4_frame)?;
        let slice = unsafe { core::slice::from_raw_parts_mut(base as *mut u8, PAGE_SIZE) };
        if frame_fill.dest_offset != 0 {
            slice[0..frame_fill.dest_offset].fill(0);
        }
        let status = mbox_get_file_page(self.last_fid, file_data.file_offset as u32, sel4_frame)
            .or(Err(seL4_InvalidArgument));
        if frame_fill.dest_len < PAGE_SIZE {
            slice[frame_fill.dest_len..].fill(0);
        }
        let _ = Self::unmap_copy_region(sel4_frame);
        status
    }

    fn mbox_map(sel4_frame: seL4_CPtr) -> seL4_Result {
        unsafe {
            let vaddr = get_mailbox_mmio().as_ptr() as usize;
            seL4_Page_Map(
                sel4_frame,
                seL4_CapInitThreadVSpace,
                vaddr,
                seL4_CapRights::new(
                    /*grant_reply=*/ 0, /*grant=*/ 0, /*read=*/ 1, /*write=*/ 1,
                ),
                seL4_Default_VMAttributes,
            )
        }
    }
    fn mbox_unmap(sel4_frame: seL4_CPtr) -> seL4_Result { unsafe { seL4_Page_Unmap(sel4_frame) } }
}
