//! Kata OS command line interface

// This brief bootstrap of Rust-in-Kata prototypes a minimal modular design
// for the DebugConsole CLI use case.
//
// * kata_io Read/Write interface (or move to std::, but that requires alloc)
// * kata_uart_client implementation of the kata_io interface
// * kata_line_reader
// * kata_shell
// * kata_debug_console main entry point fn run()

#![no_std]

use core::slice;
use kata_os_common::camkes::Camkes;

extern "C" {
    static cpio_archive: *const u8; // CPIO archive of built-in files
}

static mut CAMKES: Camkes = Camkes::new("DebugConsole");

#[no_mangle]
pub unsafe extern "C" fn pre_init() {
    // TODO(b/200946906): Review per-component heap allocations, including this one.
    const HEAP_SIZE: usize = 1 << 20;
    static mut HEAP_MEMORY: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    CAMKES.pre_init(
        log::LevelFilter::Debug,
        &mut HEAP_MEMORY,
    );
}

/// Entry point for DebugConsole. Runs the shell with UART IO.
#[no_mangle]
pub extern "C" fn run() -> ! {
    let mut tx = kata_uart_client::Tx::new();
    let mut rx = kata_io::BufReader::new(kata_uart_client::Rx::new());
    let cpio_archive_ref = unsafe {
        // XXX want begin-end or begin+size instead of a fixed-size block
        slice::from_raw_parts(cpio_archive, 16777216)
    };
    kata_shell::repl(&mut tx, &mut rx, cpio_archive_ref);
}
