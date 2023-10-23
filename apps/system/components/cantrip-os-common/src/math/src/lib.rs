// Copyright 2020 Google LLC
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

// The below definitions work for all architectures Cantrip supports, since
// floats are guaranteed to be 32-bits on all of the architectures supported
// currently (x86, aarch32, aarch64, riscv32, riscv64). This is not guaranteed
// to be the case for future architectures, though IEEE-754 is pretty explicit
// on word sizes, so we might be fine.
//
// These should be defined in a more portable way somehow, but really, they
// should be eliminated entirely from dependent code.

#[no_mangle]
fn fmax(a: f64, b: f64) -> f64 {
    a.max(b)
}

#[no_mangle]
fn fmin(a: f64, b: f64) -> f64 {
    a.min(b)
}

#[no_mangle]
fn fminf(a: f32, b: f32) -> f32 {
    a.min(b)
}

#[no_mangle]
fn fmaxf(a: f32, b: f32) -> f32 {
    a.max(b)
}
