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

// Setters and getters for the Springbok Vector Core CSRs.

use crate::Permission;
use modular_bitfield::prelude::*;
use reg_constants::vc_top::*;

unsafe fn get_vc_top(offset: usize) -> *const u32 {
    extern "Rust" {
        fn get_csr() -> &'static [u8];
    }
    get_csr().as_ptr().add(offset).cast::<u32>()
}
unsafe fn get_vc_top_mut(offset: usize) -> *mut u32 {
    extern "Rust" {
        fn get_csr_mut() -> &'static mut [u8];
    }
    get_csr_mut().as_mut_ptr().add(offset).cast::<u32>()
}

#[bitfield]
pub struct IntrState {
    pub host_req: bool,
    pub finish: bool,
    pub instruction_fault: bool,
    pub data_fault: bool,
    #[skip]
    __: B28,
}
pub fn get_intr_state() -> IntrState {
    unsafe {
        IntrState::from_bytes(
            get_vc_top(VC_TOP_INTR_STATE_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_intr_state(intr_state: IntrState) {
    unsafe {
        get_vc_top_mut(VC_TOP_INTR_STATE_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(intr_state.into_bytes()))
    }
}

#[bitfield]
pub struct IntrEnable {
    pub host_req: bool,
    pub finish: bool,
    pub instruction_fault: bool,
    pub data_fault: bool,
    #[skip]
    __: B28,
}
pub fn get_intr_enable() -> IntrEnable {
    unsafe {
        IntrEnable::from_bytes(
            get_vc_top(VC_TOP_INTR_ENABLE_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}

pub fn set_intr_enable(intr_enable: IntrEnable) {
    unsafe {
        get_vc_top_mut(VC_TOP_INTR_ENABLE_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(intr_enable.into_bytes()))
    }
}

#[bitfield]
pub struct IntrTest {
    pub host_req: bool,
    pub finish: bool,
    pub instruction_fault: bool,
    pub data_fault: bool,
    #[skip]
    __: B28,
}
pub fn get_intr_test() -> IntrTest {
    unsafe {
        IntrTest::from_bytes(
            get_vc_top(VC_TOP_INTR_TEST_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_intr_test(intr_test: IntrTest) {
    unsafe {
        get_vc_top_mut(VC_TOP_INTR_TEST_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(intr_test.into_bytes()))
    }
}

#[bitfield]
pub struct Ctrl {
    pub freeze: bool,
    pub vc_reset: bool,
    pub pc_start: B17,
    #[skip]
    __: B13,
}
pub fn get_ctrl() -> Ctrl {
    unsafe {
        Ctrl::from_bytes(
            get_vc_top(VC_TOP_CTRL_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_ctrl(ctrl: Ctrl) {
    unsafe {
        get_vc_top_mut(VC_TOP_CTRL_REG_OFFSET).write_volatile(u32::from_ne_bytes(ctrl.into_bytes()))
    }
}

#[bitfield]
pub struct MemoryBankCtrl {
    pub i_mem_enable: B4,
    pub d_mem_enable: B8,
    #[skip]
    __: B20,
}
pub fn get_memory_bank_ctrl() -> MemoryBankCtrl {
    unsafe {
        MemoryBankCtrl::from_bytes(
            get_vc_top(VC_TOP_MEMORY_BANK_CTRL_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_memory_bank_ctrl(memory_bank_ctrl: MemoryBankCtrl) {
    unsafe {
        get_vc_top_mut(VC_TOP_MEMORY_BANK_CTRL_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(memory_bank_ctrl.into_bytes()))
    }
}

#[bitfield]
pub struct ErrorStatus {
    pub i_mem_out_of_range: bool,
    pub d_mem_out_of_range: bool,
    pub i_mem_disable_access: B4,
    pub d_mem_disable_access: B8,
    #[skip]
    __: B18,
}
pub fn get_error_status() -> ErrorStatus {
    unsafe {
        ErrorStatus::from_bytes(
            get_vc_top(VC_TOP_ERROR_STATUS_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_error_status(error_status: ErrorStatus) {
    unsafe {
        get_vc_top_mut(VC_TOP_ERROR_STATUS_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(error_status.into_bytes()))
    }
}

#[bitfield]
pub struct InitStart {
    pub address: B22,
    pub imem_dmem_sel: bool,
    #[skip]
    __: B9,
}
pub fn get_init_start() -> InitStart {
    unsafe {
        InitStart::from_bytes(
            get_vc_top(VC_TOP_INIT_START_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_init_start(init_start: InitStart) {
    unsafe {
        get_vc_top_mut(VC_TOP_INIT_START_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(init_start.into_bytes()))
    }
}

#[bitfield]
pub struct InitEnd {
    pub address: B22,
    pub valid: bool,
    #[skip]
    __: B9,
}
pub fn get_init_end() -> InitEnd {
    unsafe {
        InitEnd::from_bytes(
            get_vc_top(VC_TOP_INIT_END_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_init_end(init_end: InitEnd) {
    unsafe {
        get_vc_top_mut(VC_TOP_INIT_END_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(init_end.into_bytes()))
    }
}

#[bitfield]
pub struct InitStatus {
    pub init_pending: bool,
    pub init_done: bool,
    #[skip]
    __: B30,
}
pub fn get_init_status() -> InitStatus {
    unsafe {
        InitStatus::from_bytes(
            get_vc_top(VC_TOP_INIT_STATUS_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_init_status(init_status: InitStatus) {
    unsafe {
        get_vc_top_mut(VC_TOP_INIT_STATUS_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(init_status.into_bytes()))
    }
}

// The WMMU registers start at 0x400 past the vector core CSRs and are 0x400
// long. Within the block, the registers are arranged like this:
// 0x0000: Window 0 Offset
// 0x0004: Window 0 Length
// 0x0008: Window 0 Permissions
// 0x000C: Unused
// 0x0010: Window 1 Offset
// 0x0014: Window 1 Length
// 0x0018: Window 1 Permissions
// 0x001C: Unused
// And so on.
const WMMU_OFFSET: usize = 0x400; // From base CSR.

const OFFSET_ADDR: usize = 0;
const LENGTH_ADDR: usize = 4;
const PERMISSIONS_ADDR: usize = 8;
const BYTES_PER_WINDOW: usize = 0x10;

const MAX_WINDOW: usize = 0x40;

unsafe fn window_ptr_mut(window: usize) -> *mut u8 {
    extern "Rust" {
        fn get_csr_mut() -> &'static mut [u8];
    }
    assert!(window < MAX_WINDOW, "Window out of range of WMMU");
    get_csr_mut()
        .as_mut_ptr()
        .add(WMMU_OFFSET + (window * BYTES_PER_WINDOW))
}

pub fn set_mmu_window_offset(window: usize, offset: usize) {
    unsafe {
        window_ptr_mut(window)
            .add(OFFSET_ADDR)
            .cast::<usize>()
            .write_volatile(offset);
    }
}

pub fn set_mmu_window_length(window: usize, length: usize) {
    unsafe {
        window_ptr_mut(window)
            .add(LENGTH_ADDR)
            .cast::<usize>()
            .write_volatile(length);
    }
}

pub fn set_mmu_window_permission(window: usize, permission: Permission) {
    unsafe {
        window_ptr_mut(window)
            .add(PERMISSIONS_ADDR)
            .cast::<usize>()
            .write_volatile(permission.bits() as usize);
    }
}

#[cfg(test)]
mod vc_tests {
    use super::*;

    // Validate modular_bitfield defs against regotool-generated SOT.

    fn bit(x: u32) -> u32 { 1 << x }
    fn bit_mask(width: u32) -> u32 { bit(width) - 1 }
    fn field(v: u32, mask: u32, shift: usize) -> u32 { (v & mask) << shift }

    #[test]
    fn intr_state() {
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_host_req(true).into_bytes()),
            bit(VC_TOP_INTR_STATE_HOST_REQ_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_finish(true).into_bytes()),
            bit(VC_TOP_INTR_STATE_FINISH_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_instruction_fault(true).into_bytes()),
            bit(VC_TOP_INTR_STATE_INSTRUCTION_FAULT_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_data_fault(true).into_bytes()),
            bit(VC_TOP_INTR_STATE_DATA_FAULT_BIT)
        );
    }
    #[test]
    fn intr_enable() {
        assert_eq!(
            u32::from_ne_bytes(IntrEnable::new().with_host_req(true).into_bytes()),
            bit(VC_TOP_INTR_ENABLE_HOST_REQ_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrEnable::new().with_finish(true).into_bytes()),
            bit(VC_TOP_INTR_ENABLE_FINISH_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrEnable::new().with_instruction_fault(true).into_bytes()),
            bit(VC_TOP_INTR_ENABLE_INSTRUCTION_FAULT_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrEnable::new().with_data_fault(true).into_bytes()),
            bit(VC_TOP_INTR_ENABLE_DATA_FAULT_BIT)
        );
    }
    #[test]
    fn intr_test() {
        assert_eq!(
            u32::from_ne_bytes(IntrTest::new().with_host_req(true).into_bytes()),
            bit(VC_TOP_INTR_TEST_HOST_REQ_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrTest::new().with_finish(true).into_bytes()),
            bit(VC_TOP_INTR_TEST_FINISH_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrTest::new().with_instruction_fault(true).into_bytes()),
            bit(VC_TOP_INTR_TEST_INSTRUCTION_FAULT_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrTest::new().with_data_fault(true).into_bytes()),
            bit(VC_TOP_INTR_TEST_DATA_FAULT_BIT)
        );
    }
    #[test]
    fn ctrl() {
        assert_eq!(
            u32::from_ne_bytes(Ctrl::new().with_freeze(true).into_bytes()),
            bit(VC_TOP_CTRL_FREEZE_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(Ctrl::new().with_vc_reset(true).into_bytes()),
            bit(VC_TOP_CTRL_VC_RESET_BIT)
        );

        assert_eq!(VC_TOP_CTRL_PC_START_MASK, bit_mask(17)); // Verify field width
        for pc in 1..VC_TOP_CTRL_PC_START_MASK {
            assert_eq!(
                u32::from_ne_bytes(Ctrl::new().with_pc_start(pc).into_bytes()),
                field(pc, VC_TOP_CTRL_PC_START_MASK, VC_TOP_CTRL_PC_START_OFFSET)
            );
        }
    }
    #[test]
    fn memory_bank_ctrl() {
        assert_eq!(VC_TOP_MEMORY_BANK_CTRL_I_MEM_ENABLE_MASK, bit_mask(4)); // Verify field width
        for mask in 1..VC_TOP_MEMORY_BANK_CTRL_I_MEM_ENABLE_MASK {
            assert_eq!(
                u32::from_ne_bytes(
                    MemoryBankCtrl::new()
                        .with_i_mem_enable(mask as u8)
                        .into_bytes()
                ),
                field(
                    mask,
                    VC_TOP_MEMORY_BANK_CTRL_I_MEM_ENABLE_MASK,
                    VC_TOP_MEMORY_BANK_CTRL_I_MEM_ENABLE_OFFSET
                )
            );
        }

        assert_eq!(VC_TOP_MEMORY_BANK_CTRL_D_MEM_ENABLE_MASK, bit_mask(8)); // Verify field width
        for mask in 1..VC_TOP_MEMORY_BANK_CTRL_D_MEM_ENABLE_MASK {
            assert_eq!(
                u32::from_ne_bytes(
                    MemoryBankCtrl::new()
                        .with_d_mem_enable(mask as u8)
                        .into_bytes()
                ),
                field(
                    mask,
                    VC_TOP_MEMORY_BANK_CTRL_D_MEM_ENABLE_MASK,
                    VC_TOP_MEMORY_BANK_CTRL_D_MEM_ENABLE_OFFSET
                )
            );
        }
    }
    #[test]
    fn error_status() {
        assert_eq!(
            u32::from_ne_bytes(
                ErrorStatus::new()
                    .with_i_mem_out_of_range(true)
                    .into_bytes()
            ),
            bit(VC_TOP_ERROR_STATUS_I_MEM_OUT_OF_RANGE_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(
                ErrorStatus::new()
                    .with_d_mem_out_of_range(true)
                    .into_bytes()
            ),
            bit(VC_TOP_ERROR_STATUS_D_MEM_OUT_OF_RANGE_BIT)
        );

        assert_eq!(VC_TOP_ERROR_STATUS_I_MEM_DISABLE_ACCESS_MASK, bit_mask(4)); // Verify field width
        for mask in 1..VC_TOP_ERROR_STATUS_I_MEM_DISABLE_ACCESS_MASK {
            assert_eq!(
                u32::from_ne_bytes(
                    ErrorStatus::new()
                        .with_i_mem_disable_access(mask as u8)
                        .into_bytes()
                ),
                field(
                    mask,
                    VC_TOP_ERROR_STATUS_I_MEM_DISABLE_ACCESS_MASK,
                    VC_TOP_ERROR_STATUS_I_MEM_DISABLE_ACCESS_OFFSET
                )
            );
        }

        assert_eq!(VC_TOP_ERROR_STATUS_D_MEM_DISABLE_ACCESS_MASK, bit_mask(8)); // Verify field width
        for mask in 1..VC_TOP_ERROR_STATUS_D_MEM_DISABLE_ACCESS_MASK {
            assert_eq!(
                u32::from_ne_bytes(
                    ErrorStatus::new()
                        .with_d_mem_disable_access(mask as u8)
                        .into_bytes()
                ),
                field(
                    mask,
                    VC_TOP_ERROR_STATUS_D_MEM_DISABLE_ACCESS_MASK,
                    VC_TOP_ERROR_STATUS_D_MEM_DISABLE_ACCESS_OFFSET
                )
            );
        }
    }
    #[test]
    fn init_start() {
        assert_eq!(VC_TOP_INIT_START_ADDRESS_MASK, bit_mask(22)); // Verify field width
        for address in 1..VC_TOP_INIT_START_ADDRESS_MASK {
            assert_eq!(
                u32::from_ne_bytes(InitStart::new().with_address(address).into_bytes()),
                field(
                    address,
                    VC_TOP_INIT_START_ADDRESS_MASK,
                    VC_TOP_INIT_START_ADDRESS_OFFSET
                )
            );
        }
        assert_eq!(
            u32::from_ne_bytes(InitStart::new().with_imem_dmem_sel(true).into_bytes()),
            bit(VC_TOP_INIT_START_IMEM_DMEM_SEL_BIT)
        );
    }
    #[test]
    fn init_end() {
        assert_eq!(VC_TOP_INIT_END_ADDRESS_MASK, bit_mask(22)); // Verify field width
        for address in 1..VC_TOP_INIT_END_ADDRESS_MASK {
            assert_eq!(
                u32::from_ne_bytes(InitEnd::new().with_address(address).into_bytes()),
                field(address, VC_TOP_INIT_END_ADDRESS_MASK, VC_TOP_INIT_END_ADDRESS_OFFSET)
            );
        }
        assert_eq!(
            u32::from_ne_bytes(InitEnd::new().with_valid(true).into_bytes()),
            bit(VC_TOP_INIT_END_VALID_BIT)
        );
    }
    fn init_status() {
        assert_eq!(
            u32::from_ne_bytes(InitStatus::new().with_init_pending(true).into_bytes()),
            bit(VC_TOP_INIT_STATUS_INIT_PENDING_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(InitStatus::new().with_init_done(true).into_bytes()),
            bit(VC_TOP_INIT_STATUS_INIT_DONE_BIT)
        );
    }
}
