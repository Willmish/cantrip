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
use log::{error, info, trace};
use sdk_interface::SDKError;
use spin::Mutex;

mod buffer;
use buffer::Buffer; // NB: buffer holds 32-bit values

#[allow(dead_code)]
mod i2s;
use i2s::*;

extern "Rust" {
    static RX_NONEMPTY: seL4_Semaphore;
    static TX_EMPTY: seL4_Semaphore;
}

use reg_constants::platform::TOP_MATCHA_SMC_I2S_CLOCK_FREQ_PERIPHERAL_HZ as CLK_FIXED_FREQ_HZ;

static RX_BUFFER: Mutex<Buffer> = Mutex::new(Buffer::new());
static mut RX_STOP_ON_FULL: bool = false; // NB: protected by RX_BUFFER
static TX_BUFFER: Mutex<Buffer> = Mutex::new(Buffer::new());

/// Resets the audio hardware according to |rxrst| and |txrst| and
/// sets the tx/rx FIFO watermark levels. Any recording or playing
/// is terminated.
pub fn audio_reset(rxrst: bool, txrst: bool, rxilvl: u8, txilvl: u8) -> Result<(), SDKError> {
    // XXX worth making errors distinct?
    fn cvt_rxilvl(rxilvl: u8) -> Result<RxILvl, SDKError> {
        match rxilvl {
            1 => Ok(RxILvl::RxLvl1),
            4 => Ok(RxILvl::RxLvl4),
            8 => Ok(RxILvl::RxLvl8),
            16 => Ok(RxILvl::RxLvl16),
            30 => Ok(RxILvl::RxLvl30),
            _ => Err(SDKError::InvalidAudioParameter),
        }
    }
    fn cvt_txilvl(txilvl: u8) -> Result<TxILvl, SDKError> {
        match txilvl {
            1 => Ok(TxILvl::TxLvl1),
            4 => Ok(TxILvl::TxLvl4),
            8 => Ok(TxILvl::TxLvl8),
            16 => Ok(TxILvl::TxLvl16),
            _ => Err(SDKError::InvalidAudioParameter),
        }
    }
    trace!("audio_reset {rxrst} {txrst} {rxilvl} {txilvl}");
    if txrst {
        let mut buf = RX_BUFFER.lock();
        audio_stop_recording(&mut buf);
    }
    if rxrst {
        let mut buf = TX_BUFFER.lock();
        audio_stop_playing(&mut buf);
    }
    set_fifo_ctrl(
        FifoCtrl::new()
            .with_rxrst(rxrst)
            .with_txrst(txrst)
            .with_rxilvl(cvt_rxilvl(rxilvl)?)
            .with_txilvl(cvt_txilvl(txilvl)?),
    );
    Ok(())
}

fn audio_drain_rx_fifo() {
    // NB: must be called with RX_BUFFER lock held
    trace!("audio_drain_rx_fifo begin");
    while rx_fifo_level() > 0 {
        let _ = get_rdata();
    }
    trace!("audio_drain_rx_fifo end");
}
fn audio_stop_recording(buf: &mut Buffer) {
    trace!("audio_stop_recording");
    // NB: must be called with RX_BUFFER lock held
    set_ctrl(get_ctrl().with_rx(false));
    set_fifo_ctrl(get_fifo_ctrl().with_rxrst(true)); // Flush RX FIFO
    set_intr_enable(get_intr_enable().with_rx_watermark(false));
    set_intr_state(get_intr_state().with_rx_watermark(false));
    audio_drain_rx_fifo();
    buf.clear();
}

pub fn audio_record_start(
    rate: usize,
    _buffer_size: usize,
    stop_on_full: bool,
) -> Result<(), SDKError> {
    fn nz(x: usize) -> usize {
        if x == 0 {
            1
        } else {
            x
        }
    }
    trace!("audio_record_start rate {rate} stop_on_full {stop_on_full}");
    let mut buf = RX_BUFFER.lock();
    let nco_rx = CLK_FIXED_FREQ_HZ / (nz(2 * rate) as u64);
    if nco_rx > reg_constants::i2s::I2S_CTRL_NCO_RX_MASK as u64 {
        error!("bad nco_rx {nco_rx} for rate {rate}");
        return Err(SDKError::InvalidAudioParameter);
    }
    // XXX or force client to stop?
//    audio_stop_recording(&mut buf);
    unsafe {
        RX_STOP_ON_FULL = stop_on_full;
    }
    set_intr_state(get_intr_state().with_rx_watermark(true));
    set_intr_enable(get_intr_enable().with_rx_watermark(true));
    set_ctrl(get_ctrl().with_rx(true).with_nco_rx(nco_rx as u8));
    Ok(())
}

pub fn audio_record_stop() -> Result<(), SDKError> {
    trace!("audio_record_stop");
    let mut buf = RX_BUFFER.lock();
    audio_stop_recording(&mut buf);
    Ok(())
}

pub fn audio_record_collect(data: &mut [u32], wait_if_empty: bool) -> Result<usize, SDKError> {
    let mut buf = RX_BUFFER.lock();
    let mut count = 0;
    while count < data.len() {
        if let Some(b) = buf.pop() {
            data[count] = b;
            count += 1;
        } else {
            // Optionally block until data is present. Note this may
            // block the caller which may block the runtime interface
            // thread which in turn may block other apps/clients.
            if wait_if_empty {
                // XXX maybe check count < data.len / 2 or similar?
                while buf.is_empty() {
                    drop(buf);
                    unsafe {
                        RX_NONEMPTY.wait();
                    }
                    buf = RX_BUFFER.lock();
                }
            } else {
                break;
            }
        }
    }
    Ok(count)
}

pub fn audio_play_start(rate: usize, _buffer_size: usize) -> Result<(), SDKError> {
    fn nz(x: usize) -> usize {
        if x == 0 {
            1
        } else {
            x
        }
    }
    trace!("audio_play_start {rate}");
    let mut buf = TX_BUFFER.lock();
    let nco_tx = CLK_FIXED_FREQ_HZ / (nz(2 * rate) as u64);
    if nco_tx > reg_constants::i2s::I2S_CTRL_NCO_TX_MASK as u64 {
        error!("bad nco_tx {nco_tx} for rate {rate}");
        return Err(SDKError::InvalidAudioParameter);
    }
    // XXX or force client to stop?
    buf.clear();
//    audio_stop_playing(&mut buf);
    set_intr_state(get_intr_state().with_tx_watermark(true));
    set_intr_enable(get_intr_enable().with_tx_watermark(true));
    set_ctrl(get_ctrl().with_tx(true).with_nco_tx(nco_tx as u8));
    Ok(())
}

pub fn audio_play_stop() -> Result<(), SDKError> {
    trace!("audio_play_stop");
    let mut buf = TX_BUFFER.lock();
    // XXX client may want to flush instead of waiting
    while !buf.is_empty() || tx_fifo_level() > 0 {
        fill_tx_fifo(&mut buf);
        drop(buf);
        unsafe {
            // XXX TxWatermark posts when buf is empty
            TX_EMPTY.wait();
        }
        buf = TX_BUFFER.lock();
    }
    audio_stop_playing(&mut buf);
    Ok(())
}

fn tx_fifo_level() -> u32 { get_fifo_status().txlvl().into() }
fn rx_fifo_level() -> u32 { get_fifo_status().rxlvl().into() }

pub fn audio_play_write(data: &[u32]) -> Result<(), SDKError> {
    trace!("play write {}", data.len());
    let mut buf = TX_BUFFER.lock();
    for ix in 0..data.len() {
        while buf.available_space() == 0 {
            trace!(
                "wait for tx_watermark {ix} avail {} fifo {}",
                buf.available_space(),
                tx_fifo_level()
            );
            drop(buf);
            unsafe {
                TX_EMPTY.wait();
            }
            buf = TX_BUFFER.lock();
            trace!(
                "tx_watermark wakeup avail {} fifo {}",
                buf.available_space(),
                tx_fifo_level()
            );
        }
        buf.push(data[ix]);
    }
    if !buf.is_empty() {
        fill_tx_fifo(&mut buf);
    }
    Ok(())
}

/// Copies from TX_BUFFER into the transmit FIFO.
///
/// This stops when the transmit FIFO is full or when TX_BUFFER is empty,
/// whichever comes first.
fn fill_tx_fifo(buf: &mut Buffer) {
    const I2S_TX_FIFO_CAPACITY: u32 = 32;

    trace!("fill_tx_fifo {} buf {}", tx_fifo_level(), buf.available_data());
    while tx_fifo_level() < I2S_TX_FIFO_CAPACITY {
        if let Some(b) = buf.pop() {
            set_wdata(b);
        } else {
            break;
        }
    }
}

fn audio_stop_playing(buf: &mut Buffer) {
    // NB: caller must drain buffer
    assert!(buf.is_empty());
    set_ctrl(get_ctrl().with_tx(false));
    set_fifo_ctrl(get_fifo_ctrl().with_txrst(true)); // Flush TX FIFO
    set_intr_state(get_intr_state().with_tx_watermark(false));
    set_intr_enable(get_intr_enable().with_tx_watermark(false));
}

// IRQ Support.

// NB: glue'd into irq framework by I2SRxWatermarkInterfaceThread
pub struct RxWatermarkInterfaceThread;
impl RxWatermarkInterfaceThread {
    pub fn handler() {
        trace!("rx_watermark begin");
        // Drain the RX fifo; data goes to the RX_BUFFER.
        let mut buf = RX_BUFFER.lock();
        if unsafe { RX_STOP_ON_FULL } {
            while rx_fifo_level() > 0 && buf.available_space() > 0 {
                buf.push(get_rdata());
            }
        } else {
            while rx_fifo_level() > 0 {
                buf.push(get_rdata());
            }
        }
        if !buf.is_empty() {
            // Data is available, wakeup any waiters.
            unsafe {
                RX_NONEMPTY.post();
            }
        }
        set_intr_state(get_intr_state().with_rx_watermark(true));
        trace!(
            "rx_watermark end, fifo {} buf {}",
            rx_fifo_level(),
            buf.available_data()
        );
    }
}

pub struct TxWatermarkInterfaceThread;
impl TxWatermarkInterfaceThread {
    pub fn handler() {
        trace!("handle tx_watermark");
        let mut buf = TX_BUFFER.lock();
        fill_tx_fifo(&mut buf);
        if buf.available_space() >= 16 {
            unsafe {
                TX_EMPTY.post();
            }
        }
        set_intr_state(get_intr_state().with_tx_watermark(true));
        trace!(
            "tx_watermark end, fifo {} buf {}",
            tx_fifo_level(),
            buf.available_data()
        );
    }
}
