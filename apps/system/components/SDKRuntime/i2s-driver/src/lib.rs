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
#![no_std]

use cantrip_os_common::camkes::semaphore::seL4_Semaphore;
use log::trace;

#[allow(dead_code)]
mod i2s;
use i2s::*;

extern "Rust" {
    static I2S_RX: seL4_Semaphore;
}

// IRQ Support.

pub struct RxWatermarkInterfaceThread;
impl RxWatermarkInterfaceThread {
    pub fn init() {
        // Clear the rx fifo & set tx fifo level to max
        set_fifo_ctrl(
            FifoCtrl::new()
                .with_rxrst(true)
                .with_txilvl(TxILvl::TxLvl16),
        );
        set_intr_state(IntrState::new().with_rx_watermark(true));
        set_intr_enable(IntrEnable::new().with_rx_watermark(true));
    }
    pub fn handler() {
        trace!("handle rx_watermark");
        set_intr_state(IntrState::new().with_rx_watermark(true));
        unsafe { &I2S_RX.post() }; // Unblock anyone waiting.
    }
}

pub struct TxEmptyInterfaceThread;
impl TxEmptyInterfaceThread {
    pub fn handler() {
        trace!("handle tx_empty");
        // Nothing to do (yet), just clear the interrupt.
        set_intr_state(IntrState::new().with_tx_empty(true));
    }
}

pub struct TxWatermarkInterfaceThread;
impl TxWatermarkInterfaceThread {
    pub fn handler() {
        trace!("handle tx_watermark");
        // Nothing to do (yet), just clear the interrupt.
        set_intr_state(IntrState::new().with_tx_watermark(true));
    }
}
