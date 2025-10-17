use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap();
    
    println!("cargo:rerun-if-changed=build.rs");
    
    // Only provide memory.x for ARM Cortex-M targets
    if target.starts_with("thumbv") {
        let memory_x = if target == "thumbv6m-none-eabi" {
            // RP Pico (RP2040)
            r#"
MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100
    RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}

EXTERN(BOOT2_FIRMWARE)

SECTIONS {
    .boot2 ORIGIN(BOOT2) :
    {
        KEEP(*(.boot2));
    } > BOOT2
} INSERT BEFORE .text;
"#
        } else if target == "thumbv7em-none-eabihf" {
            // micro:bit v2 (nRF52833)
            r#"
MEMORY
{
  FLASH : ORIGIN = 0x00000000, LENGTH = 512K
  RAM : ORIGIN = 0x20000000, LENGTH = 128K
}
"#
        } else {
            return; // No memory.x needed for other targets
        };
        
        fs::write(out_dir.join("memory.x"), memory_x).unwrap();
        println!("cargo:rustc-link-search={}", out_dir.display());
    }
}
