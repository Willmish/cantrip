/* Copyright (c) 2015 The Robigalia Project Developers
 * Licensed under the Apache License, Version 2.0
 * <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT
 * license <LICENSE-MIT or http://opensource.org/licenses/MIT>,
 * at your option. All files in the project carrying such
 * notice may not be copied, modified, or distributed except
 * according to those terms.
 */

use std::env;
use std::fs::File;
use std::os::unix::prelude::*;
use std::process::{Command, Stdio};
use std::vec::Vec;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    // Default to python3 (maybe necessary for code divergence)
    let python_bin = env::var("PYTHON").unwrap_or_else(|_| "python3".to_string());

    // Default to "seL4" for backwards compat; can either use git submodule or
    // symbolic link (neither recommended)
    let sel4_dir = env::var("SEL4_DIR").unwrap_or_else(|_| "seL4".to_string());

    // Default to full list of architectures but only riscv32 tested (and
    // others have not been updated for latest seL4 kernel; e.g. mcs api)
    let arch_list =
        env::var("SEL4_ARCHES").unwrap_or_else(|_| "ia32,x86_64,aarch32,riscv32".to_string());
    let mut arches = Vec::new();
    for arch in arch_list.split(',') {
        match arch {
            "ia32" => arches.push(("ia32", "x86", 32)),
            "x86_64" => arches.push(("x86_64", "x86", 64)),
            "aarch32" => arches.push(("aarch32", "arm", 32)),
            "riscv32" => arches.push(("riscv32", "riscv", 32)),
            "riscv64" => arches.push(("riscv64", "riscv", 64)),
            _ => panic!("Unsupported architecture {}", arch),
        }
    }

    let xml_interfaces_file = format!("{}/libsel4/include/interfaces/sel4.xml", sel4_dir);
    for &(arch, archdir, word_size) in &arches {
        let word_size = format!("{}", word_size);
        let outfile = format!("{}/{}_syscall_stub.rs", out_dir, arch);
        let xml_arch_file = &*format!(
            "{}/libsel4/arch_include/{}/interfaces/sel4arch.xml",
            sel4_dir, archdir
        );
        let xml_sel4_arch_file = format!(
            "{}/libsel4/sel4_arch_include/{}/interfaces/sel4arch.xml",
            sel4_dir, arch
        );
        let args = vec![
            "tools/syscall_stub_gen.py",
            "-a",
            arch,
            "-w",
            &*word_size,
            "--buffer",
            #[cfg(feature = "CONFIG_KERNEL_MCS")]
            "--mcs",
            "-o",
            &*outfile,
            &*xml_interfaces_file,
            &*xml_arch_file,
            &*xml_sel4_arch_file,
        ];

        let mut cmd = Command::new("/usr/bin/env");
        cmd.arg(&python_bin).args(&args);

        println!("Running: {:?}", cmd);
        assert!(cmd.status().unwrap().success());
    }

    // TODO(sleffler): requires pip install tempita
    for &(arch, archdir, _word_size) in &arches {
        let xml_arch_file = &*format!(
            "{}/libsel4/arch_include/{}/interfaces/sel4arch.xml",
            sel4_dir, archdir
        );
        let xml_sel4_arch_file = format!(
            "{}/libsel4/sel4_arch_include/{}/interfaces/sel4arch.xml",
            sel4_dir, arch
        );
        let mut cmd = Command::new("/usr/bin/env");
        cmd.arg(&python_bin).args(&[
            "tools/invocation_header_gen.py",
            "--dest",
            &*format!("{}/{}_invocation.rs", out_dir, arch),
            &*xml_interfaces_file,
            &*xml_arch_file,
            &*xml_sel4_arch_file,
        ]);
        println!("Running {:?}", cmd);
        assert!(cmd.status().unwrap().success());
    }

    // TODO(sleffler): requires pip install tempita
    let mut cmd = Command::new("/usr/bin/env");
    cmd.arg(&python_bin).args(&[
        "tools/syscall_header_gen.py",
        #[cfg(feature = "CONFIG_KERNEL_MCS")]
        "--mcs",
        "--xml",
        &*format!("{}/libsel4/include/api/syscall.xml", sel4_dir),
        "--dest",
        &*format!("{}/syscalls.rs", out_dir),
    ]);
    println!("Running {:?}", cmd);
    assert!(cmd.status().unwrap().success());

    let bfin = File::open(&*format!(
        "{}/libsel4/mode_include/32/sel4/shared_types.bf",
        sel4_dir
    ))
    .unwrap();
    println!("{}/types32.rs", out_dir);
    let bfout = File::create(&*format!("{}/types32.rs", out_dir)).unwrap();
    let mut cmd = Command::new("/usr/bin/env");
    cmd.arg(&python_bin)
        .arg("tools/bitfield_gen.py")
        .arg("--language=rust")
        //       .arg("--word-size=32")
        .stdin(unsafe { Stdio::from_raw_fd(bfin.as_raw_fd()) })
        .stdout(unsafe { Stdio::from_raw_fd(bfout.as_raw_fd()) });
    println!("Running {:?}", cmd);
    assert!(cmd.status().unwrap().success());
    std::mem::forget(bfin);
    std::mem::forget(bfout);

    let bfin = File::open(&*format!(
        "{}/libsel4/mode_include/64/sel4/shared_types.bf",
        sel4_dir
    ))
    .unwrap();
    let bfout = File::create(&*format!("{}/types64.rs", out_dir)).unwrap();
    let mut cmd = Command::new("/usr/bin/env");
    cmd.arg(&python_bin)
        .arg("tools/bitfield_gen.py")
        .arg("--language=rust")
        //       .arg("--word-size=64")
        .stdin(unsafe { Stdio::from_raw_fd(bfin.as_raw_fd()) })
        .stdout(unsafe { Stdio::from_raw_fd(bfout.as_raw_fd()) });
    println!("Running {:?}", cmd);
    assert!(cmd.status().unwrap().success());
    std::mem::forget(bfin);
    std::mem::forget(bfout);
}
