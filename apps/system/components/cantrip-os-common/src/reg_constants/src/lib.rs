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

pub mod i2s {
    include!(concat!(env!("OUT_DIR"), "/i2s.rs"));
}

pub mod mailbox {
    include!(concat!(env!("OUT_DIR"), "/mailbox.rs"));
}

pub mod ml_top {
    include!(concat!(env!("OUT_DIR"), "/ml_top.rs"));
}

pub mod timer {
    include!(concat!(env!("OUT_DIR"), "/timer.rs"));
}

pub mod uart {
    include!(concat!(env!("OUT_DIR"), "/uart.rs"));
}

pub mod vc_top {
    include!(concat!(env!("OUT_DIR"), "/vc_top.rs"));
}

#[cfg(feature = "CONFIG_PLAT_SHODAN")]
pub mod platform {
    include!("plat_shodan.rs");
}

#[cfg(feature = "CONFIG_PLAT_NEXUS")]
pub mod platform {
    include!("plat_nexus.rs");
}
