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

// Helpers to read/write I2S MMIO registers.

use modular_bitfield::prelude::*;
use reg_constants::i2s::*;

// Glue for I2S hw access.

pub unsafe fn get_i2s(offset: usize) -> *const u32 {
    extern "Rust" {
        fn get_i2s_csr() -> &'static [u8];
    }
    get_i2s_csr().as_ptr().add(offset).cast::<u32>()
}
pub unsafe fn get_i2s_mut(offset: usize) -> *mut u32 {
    extern "Rust" {
        fn get_i2s_csr_mut() -> &'static mut [u8];
    }
    get_i2s_csr_mut().as_mut_ptr().add(offset).cast::<u32>()
}

// Interrupt State register.
#[bitfield]
pub struct IntrState {
    pub tx_watermark: bool,
    pub rx_watermark: bool,
    pub tx_empty: bool,
    pub rx_overflow: bool,
    #[skip]
    __: B28,
}
pub fn get_intr_state() -> IntrState {
    unsafe {
        IntrState::from_bytes(
            get_i2s(I2S_INTR_STATE_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_intr_state(state: IntrState) {
    unsafe {
        get_i2s_mut(I2S_INTR_STATE_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(state.into_bytes()))
    }
}

// Interrupt Enable register.
#[bitfield]
pub struct IntrEnable {
    pub tx_watermark: bool,
    pub rx_watermark: bool,
    pub tx_empty: bool,
    pub rx_overflow: bool,
    #[skip]
    __: B28,
}
pub fn get_intr_enable() -> IntrEnable {
    unsafe {
        IntrEnable::from_bytes(
            get_i2s(I2S_INTR_ENABLE_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_intr_enable(enable: IntrEnable) {
    unsafe {
        get_i2s_mut(I2S_INTR_ENABLE_REG_OFFSET)
            .write_volatile(u32::from_ne_bytes(enable.into_bytes()))
    }
}

// Interrupt Test register.
#[bitfield]
pub struct IntrTest {
    pub tx_watermark: bool,
    pub rx_watermark: bool,
    pub tx_empty: bool,
    pub rx_overflow: bool,
    #[skip]
    __: B28,
}
pub fn get_intr_test() -> IntrTest {
    unsafe {
        IntrTest::from_bytes(
            get_i2s(I2S_INTR_TEST_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_intr_test(test: IntrTest) {
    unsafe {
        get_i2s_mut(I2S_INTR_TEST_REG_OFFSET).write_volatile(u32::from_ne_bytes(test.into_bytes()))
    }
}

// I2S control register.
#[bitfield]
pub struct Ctrl {
    pub tx: bool,    // tx enable
    pub rx: bool,    // rx enable
    pub slpbk: bool, // system loopback enable
    #[skip]
    __: B15,
    pub nco_rx: B7, // rx control clock divider
    pub nco_tx: B7, // tx control clock divider
}
pub fn get_ctrl() -> Ctrl {
    unsafe { Ctrl::from_bytes(get_i2s(I2S_CTRL_REG_OFFSET).read_volatile().to_ne_bytes()) }
}
pub fn set_ctrl(ctrl: Ctrl) {
    unsafe {
        get_i2s_mut(I2S_CTRL_REG_OFFSET).write_volatile(u32::from_ne_bytes(ctrl.into_bytes()))
    }
}

// I2S Status register.
#[bitfield]
pub struct Status {
    pub txfull: bool,
    pub rxfull: bool,
    pub txempty: bool,
    pub rxempty: bool,
    #[skip]
    __: B28,
}
pub fn get_status() -> Status {
    unsafe { Status::from_bytes(get_i2s(I2S_STATUS_REG_OFFSET).read_volatile().to_ne_bytes()) }
}
pub fn set_status(status: Status) {
    unsafe {
        get_i2s_mut(I2S_STATUS_REG_OFFSET).write_volatile(u32::from_ne_bytes(status.into_bytes()))
    }
}

// I2S read data.
pub fn get_rdata() -> u32 { unsafe { get_i2s(I2S_RDATA_REG_OFFSET).read_volatile() } }
// NB: read-only

// I2S write data.
pub fn set_wdata(data: u32) { unsafe { get_i2s_mut(I2S_WDATA_REG_OFFSET).write_volatile(data) } }
// NB: write-only

#[repr(u32)]
#[derive(BitfieldSpecifier)]
#[bits = 3]
pub enum RxILvl {
    RxLvl1 = I2S_FIFO_CTRL_RXILVL_VALUE_RXLVL1,
    RxLvl4 = I2S_FIFO_CTRL_RXILVL_VALUE_RXLVL4,
    RxLvl8 = I2S_FIFO_CTRL_RXILVL_VALUE_RXLVL8,
    RxLvl16 = I2S_FIFO_CTRL_RXILVL_VALUE_RXLVL16,
    RxLvl30 = I2S_FIFO_CTRL_RXILVL_VALUE_RXLVL30,
}

#[repr(u32)]
#[derive(BitfieldSpecifier)]
#[bits = 2]
pub enum TxILvl {
    TxLvl1 = I2S_FIFO_CTRL_TXILVL_VALUE_TXLVL1,
    TxLvl4 = I2S_FIFO_CTRL_TXILVL_VALUE_TXLVL4,
    TxLvl8 = I2S_FIFO_CTRL_TXILVL_VALUE_TXLVL8,
    TxLvl16 = I2S_FIFO_CTRL_TXILVL_VALUE_TXLVL16,
}

// I2S FIFO control register.
#[bitfield]
pub struct FifoCtrl {
    pub rxrst: bool,
    pub txrst: bool,
    pub rxilvl: RxILvl,
    pub txilvl: TxILvl,
    #[skip]
    __: B25,
}
pub fn get_fifo_ctrl() -> FifoCtrl {
    unsafe {
        FifoCtrl::from_bytes(
            get_i2s(I2S_FIFO_CTRL_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
pub fn set_fifo_ctrl(ctrl: FifoCtrl) {
    unsafe {
        get_i2s_mut(I2S_FIFO_CTRL_REG_OFFSET).write_volatile(u32::from_ne_bytes(ctrl.into_bytes()))
    }
}

// I2S FIFO status register.
#[bitfield]
pub struct FifoStatus {
    pub txlvl: B6,
    #[skip]
    __: B10,
    pub rxlvl: B6,
    #[skip]
    __: B10,
}
pub fn get_fifo_status() -> FifoStatus {
    unsafe {
        FifoStatus::from_bytes(
            get_i2s(I2S_FIFO_STATUS_REG_OFFSET)
                .read_volatile()
                .to_ne_bytes(),
        )
    }
}
// NB: read-only

#[cfg(test)]
mod tests {
    use super::*;

    // Validate modular_bitfield defs against regotool-generated SOT.

    fn bit(x: u32) -> u32 { 1 << x }
    fn field(v: u32, mask: u32, shift: usize) -> u32 { (v & mask) << shift }

    #[test]
    fn intr_state() {
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_tx_watermark(true).into_bytes()),
            bit(I2S_INTR_STATE_TX_WATERMARK_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_rx_watermark(true).into_bytes()),
            bit(I2S_INTR_STATE_RX_WATERMARK_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_tx_empty(true).into_bytes()),
            bit(I2S_INTR_STATE_TX_EMPTY_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_rx_overflow(true).into_bytes()),
            bit(I2S_INTR_STATE_RX_OVERFLOW_BIT)
        );
    }
    #[test]
    fn intr_enable() {
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_tx_watermark(true).into_bytes()),
            bit(I2S_INTR_ENABLE_TX_WATERMARK_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_rx_watermark(true).into_bytes()),
            bit(I2S_INTR_ENABLE_RX_WATERMARK_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_tx_empty(true).into_bytes()),
            bit(I2S_INTR_ENABLE_TX_EMPTY_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_rx_overflow(true).into_bytes()),
            bit(I2S_INTR_ENABLE_RX_OVERFLOW_BIT)
        );
    }
    #[test]
    fn intr_test() {
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_tx_watermark(true).into_bytes()),
            bit(I2S_INTR_TEST_TX_WATERMARK_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_rx_watermark(true).into_bytes()),
            bit(I2S_INTR_TEST_RX_WATERMARK_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_tx_empty(true).into_bytes()),
            bit(I2S_INTR_TEST_TX_EMPTY_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(IntrState::new().with_rx_overflow(true).into_bytes()),
            bit(I2S_INTR_TEST_RX_OVERFLOW_BIT)
        );
    }
    #[test]
    fn ctrl() {
        assert_eq!(
            u32::from_ne_bytes(Ctrl::new().with_tx(true).into_bytes()),
            bit(I2S_CTRL_TX_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(Ctrl::new().with_rx(true).into_bytes()),
            bit(I2S_CTRL_RX_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(Ctrl::new().with_slpbk(true).into_bytes()),
            bit(I2S_CTRL_SLPBK_BIT)
        );
        for nco_rx in 1..I2S_CTRL_NCO_RX_MASK {
            assert_eq!(
                u32::from_ne_bytes(Ctrl::new().with_nco_rx(nco_rx as u8).into_bytes()),
                field(nco_rx, I2S_CTRL_NCO_RX_MASK, I2S_CTRL_NCO_RX_OFFSET)
            );
        }
        for nco_tx in 1..I2S_CTRL_NCO_TX_MASK {
            assert_eq!(
                u32::from_ne_bytes(Ctrl::new().with_nco_tx(nco_tx as u8).into_bytes()),
                field(nco_tx, I2S_CTRL_NCO_TX_MASK, I2S_CTRL_NCO_TX_OFFSET)
            );
        }
    }
    #[test]
    fn status() {
        assert_eq!(
            u32::from_ne_bytes(Status::new().with_txfull(true).into_bytes()),
            bit(I2S_STATUS_TXFULL_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(Status::new().with_rxfull(true).into_bytes()),
            bit(I2S_STATUS_RXFULL_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(Status::new().with_txempty(true).into_bytes()),
            bit(I2S_STATUS_TXEMPTY_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(Status::new().with_rxempty(true).into_bytes()),
            bit(I2S_STATUS_RXEMPTY_BIT)
        );
    }
    #[test]
    fn fifo_ctrl() {
        assert_eq!(
            u32::from_ne_bytes(FifoCtrl::new().with_rxrst(true).into_bytes()),
            bit(I2S_FIFO_CTRL_RXRST_BIT)
        );
        assert_eq!(
            u32::from_ne_bytes(FifoCtrl::new().with_txrst(true).into_bytes()),
            bit(I2S_FIFO_CTRL_TXRST_BIT)
        );

        assert_eq!(
            u32::from_ne_bytes(FifoCtrl::new().with_rxilvl(RxILvl::RxLvl1).into_bytes()),
            field(
                I2S_FIFO_CTRL_RXILVL_VALUE_RXLVL1,
                I2S_FIFO_CTRL_RXILVL_MASK,
                I2S_FIFO_CTRL_RXILVL_OFFSET
            )
        );
        assert_eq!(
            u32::from_ne_bytes(FifoCtrl::new().with_rxilvl(RxILvl::RxLvl4).into_bytes()),
            field(
                I2S_FIFO_CTRL_RXILVL_VALUE_RXLVL4,
                I2S_FIFO_CTRL_RXILVL_MASK,
                I2S_FIFO_CTRL_RXILVL_OFFSET
            )
        );
        assert_eq!(
            u32::from_ne_bytes(FifoCtrl::new().with_rxilvl(RxILvl::RxLvl8).into_bytes()),
            field(
                I2S_FIFO_CTRL_RXILVL_VALUE_RXLVL8,
                I2S_FIFO_CTRL_RXILVL_MASK,
                I2S_FIFO_CTRL_RXILVL_OFFSET
            )
        );
        assert_eq!(
            u32::from_ne_bytes(FifoCtrl::new().with_rxilvl(RxILvl::RxLvl16).into_bytes()),
            field(
                I2S_FIFO_CTRL_RXILVL_VALUE_RXLVL16,
                I2S_FIFO_CTRL_RXILVL_MASK,
                I2S_FIFO_CTRL_RXILVL_OFFSET
            )
        );
        assert_eq!(
            u32::from_ne_bytes(FifoCtrl::new().with_rxilvl(RxILvl::RxLvl30).into_bytes()),
            field(
                I2S_FIFO_CTRL_RXILVL_VALUE_RXLVL30,
                I2S_FIFO_CTRL_RXILVL_MASK,
                I2S_FIFO_CTRL_RXILVL_OFFSET
            )
        );

        assert_eq!(
            u32::from_ne_bytes(FifoCtrl::new().with_txilvl(TxILvl::TxLvl1).into_bytes()),
            field(
                I2S_FIFO_CTRL_TXILVL_VALUE_TXLVL1,
                I2S_FIFO_CTRL_TXILVL_MASK,
                I2S_FIFO_CTRL_TXILVL_OFFSET
            )
        );
        assert_eq!(
            u32::from_ne_bytes(FifoCtrl::new().with_txilvl(TxILvl::TxLvl4).into_bytes()),
            field(
                I2S_FIFO_CTRL_TXILVL_VALUE_TXLVL4,
                I2S_FIFO_CTRL_TXILVL_MASK,
                I2S_FIFO_CTRL_TXILVL_OFFSET
            )
        );
        assert_eq!(
            u32::from_ne_bytes(FifoCtrl::new().with_txilvl(TxILvl::TxLvl8).into_bytes()),
            field(
                I2S_FIFO_CTRL_TXILVL_VALUE_TXLVL8,
                I2S_FIFO_CTRL_TXILVL_MASK,
                I2S_FIFO_CTRL_TXILVL_OFFSET
            )
        );
        assert_eq!(
            u32::from_ne_bytes(FifoCtrl::new().with_txilvl(TxILvl::TxLvl16).into_bytes()),
            field(
                I2S_FIFO_CTRL_TXILVL_VALUE_TXLVL16,
                I2S_FIFO_CTRL_TXILVL_MASK,
                I2S_FIFO_CTRL_TXILVL_OFFSET
            )
        );
    }
    #[test]
    fn fifo_status() {
        for level in 1..I2S_FIFO_STATUS_TXLVL_MASK {
            assert_eq!(
                u32::from_ne_bytes(FifoStatus::new().with_txlvl(level as u8).into_bytes()),
                field(level, I2S_FIFO_STATUS_TXLVL_MASK, I2S_FIFO_STATUS_TXLVL_OFFSET)
            );
        }
        for level in 1..I2S_FIFO_STATUS_RXLVL_MASK {
            assert_eq!(
                u32::from_ne_bytes(FifoStatus::new().with_rxlvl(level as u8).into_bytes()),
                field(level, I2S_FIFO_STATUS_RXLVL_MASK, I2S_FIFO_STATUS_RXLVL_OFFSET)
            );
        }
    }
}
