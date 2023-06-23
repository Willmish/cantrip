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

// Helpers to read/write Mailbox MMIO registers.

use modular_bitfield::prelude::*;
use reg_constants::mailbox::*;

// NB: these assume MAILBOX_MMIO is visible in the top-level crate;
// if not, then use get_mailbox_mmio()
unsafe fn get_mbox(offset: usize) -> *const u32 {
    crate::MAILBOX_MMIO.data.as_ptr().add(offset).cast::<u32>()
}
unsafe fn get_mbox_mut(offset: usize) -> *mut u32 {
    crate::MAILBOX_MMIO
        .data
        .as_mut_ptr()
        .add(offset)
        .cast::<u32>()
}

// Interrupt State register.
#[bitfield]
pub struct IntrState {
    pub wtirq: bool,
    pub rtirq: bool,
    pub eirq: bool,
    #[skip]
    __: B29,
}
pub fn get_intr_state() -> IntrState {
    unsafe {
        IntrState::from_bytes(
            get_mbox(TLUL_MAILBOX_INTR_STATE_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_intr_state(state: IntrState) {
    unsafe {
        get_mbox_mut(TLUL_MAILBOX_INTR_STATE_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(state.into_bytes()))
    }
}

// Interrupt Enable register.
#[bitfield]
pub struct IntrEnable {
    pub wtirq: bool,
    pub rtirq: bool,
    pub eirq: bool,
    #[skip]
    __: B29,
}
pub fn get_intr_enable() -> IntrEnable {
    unsafe {
        IntrEnable::from_bytes(
            get_mbox(TLUL_MAILBOX_INTR_ENABLE_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_intr_enable(enable: IntrEnable) {
    unsafe {
        get_mbox_mut(TLUL_MAILBOX_INTR_ENABLE_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(enable.into_bytes()))
    }
}

// Interrupt Test register.
#[bitfield]
pub struct IntrTest {
    pub wtirq: bool,
    pub rtirq: bool,
    pub eirq: bool,
    #[skip]
    __: B29,
}
pub fn get_intr_test() -> IntrTest {
    unsafe {
        IntrTest::from_bytes(
            get_mbox(TLUL_MAILBOX_INTR_TEST_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_intr_test(test: IntrTest) {
    unsafe {
        get_mbox_mut(TLUL_MAILBOX_INTR_TEST_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(test.into_bytes()))
    }
}

// Mailbox write register address.
pub fn get_mboxw() -> u32 { unsafe { get_mbox(TLUL_MAILBOX_MBOXW_REG_OFFSET).read_volatile() } }
pub fn set_mboxw(addr: u32) {
    unsafe { get_mbox_mut(TLUL_MAILBOX_MBOXW_REG_OFFSET).write_volatile(addr) }
}

// Mailbox read register address.
pub fn get_mboxr() -> u32 { unsafe { get_mbox(TLUL_MAILBOX_MBOXR_REG_OFFSET).read_volatile() } }
pub fn set_mboxr(addr: u32) {
    unsafe { get_mbox_mut(TLUL_MAILBOX_MBOXR_REG_OFFSET).write_volatile(addr) }
}

// Mailbox Status register.
#[bitfield]
pub struct Status {
    pub empty: bool,
    pub full: bool,
    pub wfifol: bool,
    pub rfifol: bool,
    #[skip]
    __: B28,
}
pub fn get_status() -> Status {
    unsafe {
        Status::from_bytes(
            get_mbox(TLUL_MAILBOX_STATUS_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_status(status: Status) {
    unsafe {
        get_mbox_mut(TLUL_MAILBOX_STATUS_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(status.into_bytes()))
    }
}

// Mailbox Error register.
#[bitfield]
pub struct Error {
    pub read: bool,
    pub write: bool,
    #[skip]
    __: B30,
}
pub fn get_error() -> Error {
    unsafe {
        Error::from_bytes(
            get_mbox(TLUL_MAILBOX_ERROR_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_error(error: Status) {
    unsafe {
        get_mbox_mut(TLUL_MAILBOX_ERROR_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(error.into_bytes()))
    }
}

// Write interrupt request threshold register.
#[bitfield]
pub struct WirqThreshold {
    pub th: B3,
    #[skip]
    __: B29,
}
pub fn get_wirq_threshold() -> WirqThreshold {
    unsafe {
        WirqThreshold::from_bytes(
            get_mbox(TLUL_MAILBOX_WIRQT_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_wirq_threshold(threshold: WirqThreshold) {
    unsafe {
        get_mbox_mut(TLUL_MAILBOX_WIRQT_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(threshold.into_bytes()))
    }
}

// Read interrupt request threshold register.
#[bitfield]
pub struct RirqThreshold {
    pub th: B3,
    #[skip]
    __: B29,
}
pub fn get_rirq_threshold() -> RirqThreshold {
    unsafe {
        RirqThreshold::from_bytes(
            get_mbox(TLUL_MAILBOX_RIRQT_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_rirq_threshold(threshold: RirqThreshold) {
    unsafe {
        get_mbox_mut(TLUL_MAILBOX_RIRQT_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(threshold.into_bytes()))
    }
}

// Mailbox control register.
#[bitfield]
pub struct Ctrl {
    pub flush_rfifo: bool,
    pub flush_wfifo: bool,
    #[skip]
    __: B30,
}
pub fn get_ctrl() -> Ctrl {
    unsafe {
        Ctrl::from_bytes(
            get_mbox(TLUL_MAILBOX_CTRL_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_ctrl(ctrl: Ctrl) {
    unsafe {
        get_mbox_mut(TLUL_MAILBOX_CTRL_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(ctrl.into_bytes()))
    }
}
