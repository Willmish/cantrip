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

#![no_std]
#![allow(non_upper_case_globals)]
#![allow(stable_features)]
#![feature(global_asm)]
#![feature(thread_local)]

use cantrip_os_common::allocator;
use cantrip_os_common::logger::CantripLogger;
use cantrip_os_common::sel4_sys::seL4_IPCBuffer;
use core::arch::global_asm;
use sdk_interface::sdk_log;
use static_assertions::*;

// NB: this mimics the logic in build.rs
assert_cfg!(any(
    all(target_arch = "arm", target_pointer_width = "32"),
    all(target_arch = "aarch64"),
    all(target_arch = "riscv32"),
    all(target_arch = "riscv64"),
    all(target_arch = "x86"),
    all(target_arch = "x86_64"),
));

#[cfg(target_arch = "x86")]
global_asm!(include_str!("arch/x86/crt0.S"));

#[cfg(target_arch = "x86_64")]
global_asm!(include_str!("arch/x86_64/crt0.S"));

#[cfg(all(target_arch = "arm", target_pointer_width = "32"))]
global_asm!(include_str!("arch/aarch32/crt0.S"));

#[cfg(target_arch = "aarch64")]
global_asm!(include_str!("arch/aarch64/crt0.S"));

#[cfg(target_arch = "riscv32")]
global_asm!(include_str!("arch/riscv32/crt0.S"));

#[cfg(target_arch = "riscv64")]
global_asm!(include_str!("arch/riscv64/crt0.S"));

#[no_mangle]
#[thread_local]
static mut __sel4_ipc_buffer: *mut seL4_IPCBuffer = 0 as _;

// Connect the logger so panic msgs are displayed.
#[no_mangle]
pub fn logger_log(_level: u8, msg: *const cstr_core::c_char) {
    let _ = sdk_log(unsafe { cstr_core::CStr::from_ptr(msg).to_str().unwrap() });
}

pub fn sdk_init(heap: &'static mut [u8]) {
    unsafe {
        allocator::ALLOCATOR.init(heap.as_mut_ptr(), heap.len());
    }
    static CANTRIP_LOGGER: CantripLogger = CantripLogger;
    log::set_logger(&CANTRIP_LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}
