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

//! RAII wrapper for a dynamically allocated CSpace slot.

#![cfg_attr(not(test), no_std)]
#![allow(non_snake_case)]

use slot_allocator::CANTRIP_CSPACE_SLOTS;

use sel4_sys::seL4_CNode_Copy;
use sel4_sys::seL4_CNode_Delete;
use sel4_sys::seL4_CNode_Mint;
use sel4_sys::seL4_CNode_Move;
use sel4_sys::seL4_CNode_Mutate;
use sel4_sys::seL4_CPtr;
use sel4_sys::seL4_CapRights;
use sel4_sys::seL4_GetCapReceivePath;
use sel4_sys::seL4_Result;
use sel4_sys::seL4_SetCapReceivePath;
use sel4_sys::seL4_Word;
use sel4_sys::seL4_WordBits;
use sel4_sys::seL4_Error;

extern "C" {
    static SELF_CNODE: seL4_CPtr;
}

// RAII wrapper for push_recv_path.
pub struct RecvPathCleanup {
    slot: seL4_CPtr,
}
impl RecvPathCleanup {
    pub fn new() -> Self {
        Self {
            slot: unsafe { seL4_GetCapReceivePath() }.1,
        }
    }
}
impl Drop for RecvPathCleanup {
    fn drop(&mut self) { unsafe { seL4_SetCapReceivePath(SELF_CNODE, self.slot, seL4_WordBits) } }
}

pub struct CSpaceSlot {
    pub slot: seL4_CPtr,
}
impl CSpaceSlot {
    pub fn new() -> Self {
        CSpaceSlot {
            slot: unsafe { CANTRIP_CSPACE_SLOTS.alloc(1) }.expect("CSpaceSlot"),
        }
    }

    /// Releases ownership of the slot; this inhibits the normal cleanup
    /// done by drop. The slot that was being managed is returned.
    pub fn release(&mut self) -> seL4_CPtr {
        let slot = self.slot;
        self.slot = seL4_CPtr::MAX;
        slot
    }

    /// Returns the (root, index, depth) seL4 path for the slot.
    pub fn get_path(&self) -> (seL4_CPtr, seL4_CPtr, u8) {
        (unsafe { SELF_CNODE }, self.slot, seL4_WordBits as u8)
    }

    /// Temporarily sets the receive path used for receiving a capability
    /// attached to an seL4 IPC message. The previous receive path is restored
    /// on scope exit.
    #[must_use]
    pub fn push_recv_path(&self) -> RecvPathCleanup {
        let cleanup = RecvPathCleanup::new();
        unsafe { seL4_SetCapReceivePath(SELF_CNODE, self.slot, seL4_WordBits) };
        cleanup
    }

    /// Copies the specified path to our slot.
    pub fn copy_to(
        &self,
        src_root: seL4_CPtr,
        src_index: seL4_CPtr,
        src_depth: u8,
        rights: seL4_CapRights,
    ) -> seL4_Result {
        unsafe {
            seL4_CNode_Copy(
                /*dest_root=*/ SELF_CNODE,
                /*dest_index= */ self.slot,
                /*dest_depth=*/ seL4_WordBits as u8,
                src_root,
                src_index,
                src_depth,
                rights,
            )
        }
    }

    /// Copies the specified path to our slot.
    pub fn dup_to(&self, src_root: seL4_CPtr, src_index: seL4_CPtr, src_depth: u8) -> seL4_Result {
        let seL4_AllRights = seL4_CapRights::new(
            /*grant_reply=*/ 1, /*grant=*/ 1, /*read=*/ 1, /*write=*/ 1,
        );
        self.copy_to(src_root, src_index, src_depth, seL4_AllRights)
    }

    /// Mints the specified path to our slot.
    pub fn mint_to(
        &self,
        src_root: seL4_CPtr,
        src_slot: seL4_CPtr,
        src_depth: u8,
        rights: seL4_CapRights,
        badge: seL4_Word,
    ) -> seL4_Result {
        unsafe {
            seL4_CNode_Mint(
                /*dest_root=*/ SELF_CNODE,
                /*dest_index= */ self.slot,
                /*dest_depth=*/ seL4_WordBits as u8,
                src_root,
                src_slot,
                src_depth,
                rights,
                badge,
            )
        }
    }

    /// Moves the specified path to our slot.
    pub fn move_to(&self, src_root: seL4_CPtr, src_slot: seL4_CPtr, src_depth: u8) -> seL4_Result {
        unsafe {
            seL4_CNode_Move(
                /*dest_root=*/ SELF_CNODE,
                /*dest_index= */ self.slot,
                /*dest_depth=*/ seL4_WordBits as u8,
                src_root,
                src_slot,
                src_depth,
            )
        }
    }

    /// Moves our slot to the specified path.
    pub fn move_from(
        &self,
        dest_root: seL4_CPtr,
        dest_slot: seL4_CPtr,
        dest_depth: u8,
    ) -> seL4_Result {
        unsafe {
            seL4_CNode_Move(
                dest_root,
                dest_slot,
                dest_depth,
                /*src_root=*/ SELF_CNODE,
                /*src_index= */ self.slot,
                /*src_depth=*/ seL4_WordBits as u8,
            )
        }
    }

    /// Mutates the specified path to our slot.
    pub fn mutate_to(
        &self,
        src_root: seL4_CPtr,
        src_slot: seL4_CPtr,
        src_depth: u8,
        badge: seL4_Word,
    ) -> seL4_Result {
        unsafe {
            seL4_CNode_Mutate(
                /*dest_root=*/ SELF_CNODE,
                /*dest_index= */ self.slot,
                /*dest_depth=*/ seL4_WordBits as u8,
                src_root,
                src_slot,
                src_depth,
                badge,
            )
        }
    }

    /// Delete any cap in our slot.
    // NB: deleting an empty slot is a noop to seL4
    pub fn delete(&self) -> seL4_Result {
        let result_error: seL4_Error = unsafe { seL4_CNode_Delete(SELF_CNODE, self.slot, seL4_WordBits as u8).error as usize }.into();
        result_error.into()
    }
}
impl Drop for CSpaceSlot {
    fn drop(&mut self) {
        if self.slot != seL4_CPtr::MAX {
            self.delete().expect("CSpaceSlot");
            unsafe { CANTRIP_CSPACE_SLOTS.free(self.slot, 1) }
        }
    }
}
