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

extern crate alloc;
use crate::mailbox::*;
#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use cantrip_os_common::sel4_sys;
use core::mem::size_of;
use log::trace;
use num_enum::{FromPrimitive, IntoPrimitive};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use sel4_sys::seL4_CPtr;
use sel4_sys::seL4_Page_GetAddress;

/// The high bit of the message header is used to identify a message
/// with an associated page. The physical address of the page is passed
/// through the FIFO immediately following the header.
pub const HEADER_FLAG_LONG_MESSAGE: u32 = 0x80000000;

#[derive(Debug, Serialize, Deserialize)]
pub enum SECRequest<'a> {
    FindFile(&'a str),     // Find file by name -> (/*fid*/ u32, /*size*/ u32)
    GetFilePage(u32, u32), // Get page of file data -> <attached page>

    Test(/*count*/ u32), // Scribble on count words of supplied page

    #[cfg(feature = "alloc")]
    GetBuiltins, // Get package names -> Vec(String)
}

#[cfg(feature = "alloc")]
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

fn sec_request<T: DeserializeOwned>(
    request: &SECRequest,
    opt_cap: Option<seL4_CPtr>,
) -> Result<T, SECRequestError> {
    fn howmany(a: usize, b: usize) -> usize { (a + b - 1) / b }
    fn roundup(a: usize, b: usize) -> usize { howmany(a, b) * b }

    trace!("sec_request {:?} opt_cap {:?}", &request, opt_cap);

    // XXX alignment
    // XXX bigger for returning builtins
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

    #[cfg(not(feature = "rootserver"))]
    {
        // Wait for notification from the rtirq handler.
        use cantrip_os_common::camkes::semaphore::seL4_Semaphore;
        extern "Rust" {
            static RX_SEMAPHORE: seL4_Semaphore;
        }
        unsafe { RX_SEMAPHORE.wait() };
    }

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

#[cfg(feature = "alloc")]
pub fn mbox_get_builtins() -> Result<cantrip_security_interface::BundleIdArray, SECRequestError> {
    sec_request(&SECRequest::GetBuiltins, None).map(|reply: GetBuiltinsResponse| reply.names)
}

pub fn mbox_find_file(name: &str) -> Result<(u32, u32), SECRequestError> {
    sec_request(&SECRequest::FindFile(name), None)
        .map(|reply: FindFileResponse| (reply.fid, reply.size_bytes))
}

pub fn mbox_get_file_page(fid: u32, offset: u32, frame: seL4_CPtr) -> Result<(), SECRequestError> {
    sec_request(&SECRequest::GetFilePage(fid, offset), Some(frame))?;
    Ok(())
}

// Sends a message to the security core using the supplied page.
pub fn mbox_test(frame: seL4_CPtr, count: u32) -> Result<u32, SECRequestError> {
    sec_request(&SECRequest::Test(count), Some(frame))?;
    // XXX just send back count for now
    Ok(count)
}
