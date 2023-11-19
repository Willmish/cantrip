// Copyright 2023 Google LLC
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

use crate::mailbox::*;
use cantrip_os_common::camkes::semaphore::seL4_Semaphore;
use log::{error, trace};

extern "Rust" {
    static RX_SEMAPHORE: seL4_Semaphore;
}

// IRQ Support.

// WTIRQ: interrupt for outbox.count > write_threshold.
pub struct WtirqInterfaceThread;
impl WtirqInterfaceThread {
    pub fn handler() {
        trace!("handle wtirq");
        // Nothing to do (yet), just clear the interrupt.
        set_intr_state(IntrState::new().with_wtirq(true));
    }
}

// RTIRQ: interrupt for inbox.count > read_threshold.
pub struct RtirqInterfaceThread;
impl RtirqInterfaceThread {
    // XXX not called 'cuz not part of trait impl
    pub fn post_init() {
        // Set the threshold to 0 so the irq fires asap.
        set_rirq_threshold(RirqThreshold::new().with_th(0));
        set_intr_state(IntrState::new().with_rtirq(true));
        set_intr_enable(IntrEnable::new().with_rtirq(true));
    }
    pub fn handler() {
        trace!("handle rtirq");
        set_intr_state(IntrState::new().with_rtirq(true));
        unsafe {
            RX_SEMAPHORE.post();
        } // Unblock anyone waiting.
    }
}

// EIRQ: interrupt when an error occurs.
pub struct EirqInterfaceThread;
impl EirqInterfaceThread {
    pub fn handler() {
        let error = get_error();
        error!("EIRQ:: read {} write {}", error.read(), error.write());
        // Nothing to do (yet), just clear the interrupt.
        set_intr_state(IntrState::new().with_eirq(true));
    }
}
