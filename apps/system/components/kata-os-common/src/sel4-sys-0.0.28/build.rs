/* Copyright (c) 2015 The Robigalia Project Developers
 * Licensed under the Apache License, Version 2.0
 * <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT
 * license <LICENSE-MIT or http://opensource.org/licenses/MIT>,
 * at your option. All files in the project carrying such
 * notice may not be copied, modified, or distributed except
 * according to those terms.
 */

use std::fs::{OpenOptions, File};
use std::process::{Command, Stdio};
use std::os::unix::prelude::*;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let arches = [("ia32", "x86", 32), ("x86_64", "x86", 64), ("aarch32", "arm", 32)];
    for &(arch, archdir, word_size) in &arches {
        let word_size = format!("{}", word_size);
        let outfile = format!("{}/{}_syscall_stub.rs", out_dir, arch);
        let archfile = format!("seL4/libsel4/arch_include/{}/interfaces/sel4arch.xml", archdir);
        let sel4archfile = format!("seL4/libsel4/sel4_arch_include/{}/interfaces/sel4arch.xml", arch);
        let args = vec!["tools/syscall_stub_gen.py", "-a", arch, "-w", &*word_size,             
                    "--buffer",                                                             
                    "-o", &*outfile, "seL4/libsel4/include/interfaces/sel4.xml", &*archfile, &*sel4archfile];

        let mut cmd = Command::new("/usr/bin/env");
        cmd.arg("python").args(&args);
                    
        println!("Running: {:?}", cmd);
        assert!(cmd.status().unwrap().success());
    }

    for &(arch, archdir, word_size) in &arches {
        let mut cmd = Command::new("/usr/bin/env");
        cmd.arg("python")
           .args(&["tools/invocation_header_gen.py",
                 "--dest", &*format!("{}/{}_invocation.rs", out_dir, arch),
                 "seL4/libsel4/include/interfaces/sel4.xml",
                 &*format!("seL4/libsel4/arch_include/{}/interfaces/sel4arch.xml", archdir),
                 &*format!("seL4/libsel4/sel4_arch_include/{}/interfaces/sel4arch.xml", arch),
            ]);
        println!("Running {:?}", cmd);
        assert!(cmd.status().unwrap().success());
    }

    let mut cmd = Command::new("/usr/bin/env");
    cmd.arg("python")
       .args(&["tools/syscall_header_gen.py",
             "--xml", "seL4/include/api/syscall.xml",
             "--dest", &*format!("{}/syscalls.rs", out_dir)]);
    println!("Running {:?}", cmd);
    assert!(cmd.status().unwrap().success());

    let bfin = File::open("seL4/libsel4/include/sel4/types_32.bf").unwrap();
    let bfout = File::create(&*format!("{}/types32.rs", out_dir)).unwrap();
    let mut cmd = Command::new("/usr/bin/env");
    cmd.arg("python")
       .arg("tools/bitfield_gen.py")
       .arg("--word-size=32")
       .stdin(unsafe { Stdio::from_raw_fd(bfin.as_raw_fd()) })
       .stdout(unsafe { Stdio::from_raw_fd(bfout.as_raw_fd()) });
    println!("Running {:?}", cmd);
    assert!(cmd.status().unwrap().success());
    std::mem::forget(bfin);
    std::mem::forget(bfout);

    let bfin = File::open("seL4/libsel4/include/sel4/shared_types_32.bf").unwrap();
    let bfout = OpenOptions::new().append(true).open(&*format!("{}/types32.rs", out_dir)).unwrap();
    let mut cmd = Command::new("/usr/bin/env");
    cmd.arg("python")
       .arg("tools/bitfield_gen.py")
       .arg("--word-size=32")
       .stdin(unsafe { Stdio::from_raw_fd(bfin.as_raw_fd()) })
       .stdout(unsafe { Stdio::from_raw_fd(bfout.as_raw_fd()) });
    println!("Running {:?}", cmd);
    assert!(cmd.status().unwrap().success());
    std::mem::forget(bfin);
    std::mem::forget(bfout);

    let bfin = File::open("seL4/libsel4/include/sel4/types_64.bf").unwrap();
    let bfout = File::create(&*format!("{}/types64.rs", out_dir)).unwrap();
    let mut cmd = Command::new("/usr/bin/env");
    cmd.arg("python")
       .arg("tools/bitfield_gen.py")
       .arg("--word-size=64")
       .stdin(unsafe { Stdio::from_raw_fd(bfin.as_raw_fd()) })
       .stdout(unsafe { Stdio::from_raw_fd(bfout.as_raw_fd()) });
    println!("Running {:?}", cmd);
    assert!(cmd.status().unwrap().success());
    std::mem::forget(bfin);
    std::mem::forget(bfout);

    let bfin = File::open("seL4/libsel4/include/sel4/shared_types_64.bf").unwrap();
    let bfout = OpenOptions::new().append(true).open(&*format!("{}/types64.rs", out_dir)).unwrap();
    let mut cmd = Command::new("/usr/bin/env");
    cmd.arg("python")
       .arg("tools/bitfield_gen.py")
       .arg("--word-size=64")
       .stdin(unsafe { Stdio::from_raw_fd(bfin.as_raw_fd()) })
       .stdout(unsafe { Stdio::from_raw_fd(bfout.as_raw_fd()) });
    println!("Running {:?}", cmd);
    assert!(cmd.status().unwrap().success());
    std::mem::forget(bfin);
    std::mem::forget(bfout);
}
