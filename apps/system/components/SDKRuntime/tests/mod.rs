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

#![allow(non_camel_case_types)]
#![allow(dead_code)]

const I2S_CSR_SIZE: usize = 4096;
struct I2S_CSR {
    pub data: [u8; I2S_CSR_SIZE],
}
static mut I2S_CSR: I2S_CSR = I2S_CSR {
    data: [0u8; I2S_CSR_SIZE],
};
pub fn get_i2s_csr() -> &'static [u8] { unsafe { &I2S_CSR.data } }
pub fn get_i2s_csr_mut() -> &'static mut [u8] { unsafe { &mut I2S_CSR.data } }

include!("../i2s-driver/src/i2s.rs");
