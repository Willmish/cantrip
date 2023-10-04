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

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use cantrip_memory_interface::*;
use cantrip_os_common::camkes;
use cantrip_os_common::sel4_sys;
use cantrip_security_interface::*;
use core::mem::size_of;
use core::str;
use log::{error, trace};
use num_enum::{FromPrimitive, IntoPrimitive};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use sel4_sys::seL4_CPtr;
use sel4_sys::seL4_Page_GetAddress;

#[allow(dead_code)]
mod mailbox;
use mailbox::*;

extern "Rust" {
    static RX_SEMAPHORE: camkes::semaphore::seL4_Semaphore;
}

/// The high bit of the message header is used to identify a message
/// with an associated page. The physical address of the page is passed
/// through the FIFO immediately following the header.
pub const HEADER_FLAG_LONG_MESSAGE: u32 = 0x80000000;

#[inline]
fn howmany(a: usize, b: usize) -> usize { (a + b - 1) / b }
#[inline]
fn roundup(a: usize, b: usize) -> usize { howmany(a, b) * b }

#[derive(Debug, Serialize, Deserialize)]
pub enum SECRequest<'a> {
    GetBuiltins,           // Get package names -> Vec(String)
    FindFile(&'a str),     // Find file by name -> (/*fid*/ u32, /*size*/ u32)
    GetFilePage(u32, u32), // Get page of file data -> <attached page>

    Test(/*count*/ u32), // Scribble on count words of supplied page
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetBuiltinsResponse {
    pub names: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FindFileResponse {
    pub fid: u32,        // Unique file identifier
    pub size_bytes: u32, // File size
}

#[repr(usize)]
#[derive(Debug, Default, Eq, PartialEq, FromPrimitive, IntoPrimitive)]
pub enum SECRequestError {
    Success = 0,
    DeserializeFailed,
    SerializeFailed,
    #[default]
    UnknownError,
    PageInvalid,
    FileNotFound,
    FileOffsetInvalid,
    // Generic errors.
    SendFailed,
    RecvFailed,
}
impl From<SECRequestError> for Result<(), SECRequestError> {
    fn from(err: SECRequestError) -> Result<(), SECRequestError> {
        if err == SECRequestError::Success {
            Ok(())
        } else {
            Err(err)
        }
    }
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
        unsafe { &RX_SEMAPHORE.post() }; // Unblock anyone waiting.
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

#[inline]
fn sec_request<T: DeserializeOwned>(
    request: &SECRequest,
    opt_cap: Option<seL4_CPtr>,
) -> Result<T, SECRequestError> {
    trace!("sec_request {:?} opt_cap {:?}", &request, opt_cap);

    // XXX alignment
    let mut request_slice: [u8; 256] = [0; 256];
    let encoded_bytes = postcard::to_slice(request, &mut request_slice[..])
        .or(Err(SECRequestError::SerializeFailed))?
        .len();

    let bytes = roundup(encoded_bytes, size_of::<u32>()) as u32;
    if let Some(cptr) = opt_cap {
        let paddr = unsafe { seL4_Page_GetAddress(cptr) }.or(Err(SECRequestError::PageInvalid))?;
        enqueue(bytes | HEADER_FLAG_LONG_MESSAGE);
        enqueue(paddr as u32);
    } else {
        enqueue(bytes); // NB: no associated page
    }
    // Send serialized request through the queue.
    for word in 0..(bytes as usize / size_of::<u32>()) {
        enqueue(unsafe { request_slice.as_ptr().cast::<u32>().add(word).read() });
    }

    // Wait for notification from the rtirq handler.
    unsafe { RX_SEMAPHORE.wait() };
    let header = dequeue();
    if (header & HEADER_FLAG_LONG_MESSAGE) != 0 {
        // NB: vestige of old protocol, should never occur
        let _paddr = dequeue();
    }

    // Receive reply from the queue and deserialize.
    // NB: safe to re-use request_slice for deserialize
    let recv_bytes = header & !HEADER_FLAG_LONG_MESSAGE;
    let recv_words = howmany(recv_bytes as usize, size_of::<u32>());
    for word in 0..recv_words {
        unsafe {
            request_slice
                .as_mut_ptr()
                .cast::<u32>()
                .add(word)
                .write(dequeue())
        }
    }
    postcard::from_bytes(&request_slice[..(recv_bytes as usize)])
        .or(Err(SECRequestError::DeserializeFailed))
}

pub fn mbox_get_builtins() -> Result<BundleIdArray, SECRequestError> {
    sec_request(&SECRequest::GetBuiltins, None).map(|reply: GetBuiltinsResponse| reply.names)
}

pub fn mbox_find_file(name: &str) -> Result<(u32, u32), SECRequestError> {
    sec_request(&SECRequest::FindFile(name), None)
        .map(|reply: FindFileResponse| (reply.fid, reply.size_bytes))
}

pub fn mbox_get_file_page(fid: u32, offset: u32, frame: &ObjDesc) -> Result<(), SECRequestError> {
    sec_request(&SECRequest::GetFilePage(fid, offset), Some(frame.cptr))?;
    Ok(())
}

// Sends a message to the security core using the supplied page.
pub fn mbox_test(frame: &ObjDesc, count: u32) -> Result<u32, SECRequestError> {
    sec_request(&SECRequest::Test(count), Some(frame.cptr))?;
    // XXX just send back count for now
    Ok(count)
}

// Directly manipulate the hardware FIFOs. Synchronous and busy-waits.
// Not thread-safe (NB: current usage is single-threaded).

fn enqueue(x: u32) {
    while get_status().full() {}
    set_mboxw(x);
}
fn dequeue() -> u32 {
    while get_status().empty() {}
    get_mboxr()
}
