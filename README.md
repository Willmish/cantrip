# Project Shodan: KataOS

Shodan is a project to build a low-power secure embedded platform
for Ambient ML applications. The target platform leverages
[RISC-V](https://riscv.org/) and [OpenTitan](https://opentitan.org/).

The Shodan
software includes a home-grown operating system named KataOS, that runs
on top of [seL4](https://github.com/seL4) and (ignoring the seL4 kernel)
is written almost entirely in [Rust](https://www.rust-lang.org/).

This is a CAmkES project that assembles the entire KataOS. It exists outside
the seL4 source trees since it contains code not intended to go to upstream
seL4.

This uses the [standard CAmkES build system](https://docs.sel4.systems/projects/camkes/manual.html#running-a-simple-example)
by symlinking CMakeLists.txt and assumes
the [CAmkES dependencies](https://docs.sel4.systems/projects/buildsystem/host-dependencies.html#camkes-build-dependencies)
are already installed.
It also symlinks settings.cmake, and so retains
the notion of "apps," which enables the build system to switch which assembly
it builds using the CAMKES\_APP CMake cache value. KataOS just has one app,
*system*.

## Sparrow Rust crates (what's included here).

[More software will be published as we deem it ready for sharing until eventually
all of Sparrow (software and hardware designs) will be available.]

Many KataOS Rust crates are in the *apps/system/components* directory.
Common/shared code is in *kata-os-common*:

- *allocator*: a heap allocator built on the linked-list-allocator crate
- *camkes*: support for writing CAmkES components in Rust
- *capdl*: support for reading the capDL specification generated by capDL-tool
- *copyregion*: a helper for temporarily mapping physical pages into a thread's VSpace
- *cspace-slot*: an RAII helper for the *slot-allocator*
- *logger*: seL4 integration with the Rust logger crate
- *model*: support for processing capDL; used by the kata-os-rootserver
- *panic*: an seL4-specific panic handler
- *sel4-config*: build glue for seL4 kernel configuration
- *sel4-sys*: seL4 system interfaces & glue
- *slot-allocator*: an allocator for slots in the top-level CNode

### Depending on Rust crates

To use crates from Sparrow you can reference them from a local repository or
directly from GitHub using git; e.g. in a Config.toml:
```
kata-os-common = { path = "../system/components/kata-os-common" }
kata-os-common = { git = "https://github.com/AmbiML/sparrow-kata" }
```
NB: the git usage depends on cargo's support for searching for a crate
named "kata-os-common" in the kata repo.
When using a git dependency a git tag can be used to lock the crate version.

Note that many Sparrow crates need the seL4 kernel configuration
(e.g. to know whether MCS is configured). This is handled by the
kata-os-common/sel4-config crate that is used by a build.rs to import
kernel configuration parameters as Cargo features. In a Cargo.toml create
a features manifest with the kernel parameters you need e.g.

```
[features]
default = []
# Used by sel4-config to extract kernel config
CONFIG_PRINTING = []
```

then specify build-dependencies:

```
[build-dependencies]
# build.rs depends on SEL4_OUT_DIR = "${ROOTDIR}/out/kata/kernel"
sel4-config = { path = "../../kata/apps/system/components/kata-os-common/src/sel4-config" }
```

and use a build.rs that includes at least:

```
extern crate sel4_config;
use std::env;

fn main() {
    // If SEL4_OUT_DIR is not set we expect the kernel build at a fixed
    // location relative to the ROOTDIR env variable.
    println!("SEL4_OUT_DIR {:?}", env::var("SEL4_OUT_DIR"));
    let sel4_out_dir = env::var("SEL4_OUT_DIR")
        .unwrap_or_else(|_| format!("{}/out/kata/kernel", env::var("ROOTDIR").unwrap()));
    println!("sel4_out_dir {}", sel4_out_dir);

    // Dredge seL4 kernel config for settings we need as features to generate
    // correct code: e.g. CONFIG_KERNEL_MCS enables MCS support which changes
    // the system call numbering.
    let features = sel4_config::get_sel4_features(&sel4_out_dir);
    println!("features={:?}", features);
    for feature in features {
        println!("cargo:rustc-cfg=feature=\"{}\"", feature);
    }
}
```

Note how build.rs expects an SEL4_OUT_DIR environment variable that has the path to
the top of the kernel build area. The build-sparrow.sh script sets this for you but, for
example, if you choose to run ninja directly you will need it set in your environment.

Similar to SEL4_OUT_DIR the kata-os-common/src/sel4-sys crate that has the seL4 system
call wrappers for Rust programs requires an SEL4_DIR envronment variable that has the
path to the top of the kernel sources. This also is set by build-sparrow.sh.

## Source Code Headers

Every file containing source code includes copyright and license
information. For dependent / non-Google code these are inherited from
the upstream repositories. If there are Google modifications you may find
the Google Apache license found below.

Apache header:

    Copyright 2022 Google LLC

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        https://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.
