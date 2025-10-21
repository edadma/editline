//! Raspberry Pi Pico 2 (RP2350) USB CDC REPL example using editline
//!
//! This example demonstrates line editing over USB CDC (the main USB port).
//! The Pico 2 will appear as a virtual COM port on your computer.
//!
//! To build and flash this example:
//! ```
//! cargo build --example rp_pico2_usb_repl --target thumbv8m.main-none-eabihf --no-default-features --features rp_pico2_usb --release
//! ```
//!
//! Then flash using the instructions in the README.
//!
//! Connect to the Pico 2's USB serial port at 115200 baud:
//! ```
//! minicom -D /dev/ttyACM0 -b 115200
//! ```

#![no_std]
#![no_main]

extern crate alloc;

use core::ptr::addr_of_mut;
use panic_halt as _;
use alloc_cortex_m::CortexMHeap;

use rp235x_hal::{
    clocks::init_clocks_and_plls,
    pac,
    usb::UsbBus,
    watchdog::Watchdog,
};

use usb_device::{
    prelude::*,
    class_prelude::UsbBusAllocator,
};
use usbd_serial::SerialPort;

use editline::{LineEditor, Terminal, terminals::rp_pico2_usb::UsbCdcTerminal};

// Tell the Boot ROM about our application (RP2350 requires this)
#[link_section = ".start_block"]
#[used]
pub static IMAGE_DEF: rp235x_hal::block::ImageDef = rp235x_hal::block::ImageDef::secure_exe();

// External high-speed crystal on the Pico 2 board is 12MHz
const XOSC_CRYSTAL_FREQ: u32 = 12_000_000u32;

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

// USB bus allocator (needs static lifetime)
static mut USB_BUS: Option<usb_device::bus::UsbBusAllocator<UsbBus>> = None;

#[rp235x_hal::entry]
fn main() -> ! {
    // Initialize the allocator
    const HEAP_SIZE: usize = 8192;
    static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    unsafe { ALLOCATOR.init(&raw mut HEAP as *const u8 as usize, HEAP_SIZE) }

    // Grab singleton objects
    let mut pac = pac::Peripherals::take().unwrap();
    let _core = cortex_m::Peripherals::take().unwrap();

    // Set up the watchdog driver
    let mut watchdog = Watchdog::new(pac.WATCHDOG);

    // Configure the clocks
    let clocks = init_clocks_and_plls(
        XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    // Set up timer for delays
    let mut timer = rp235x_hal::Timer::new_timer0(pac.TIMER0, &mut pac.RESETS, &clocks);

    // Set up the USB driver
    let usb_bus = UsbBusAllocator::new(UsbBus::new(
        pac.USB,
        pac.USB_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));
    unsafe {
        USB_BUS = Some(usb_bus);
    }

    let usb_bus_ref = unsafe { (*addr_of_mut!(USB_BUS)).as_ref().unwrap() };

    // Set up the USB Communications Class Device driver
    let serial = SerialPort::new(usb_bus_ref);

    // Create a USB device with a fake VID and PID
    let usb_dev = UsbDeviceBuilder::new(usb_bus_ref, UsbVidPid(0x16c0, 0x27dd))
        .strings(&[StringDescriptors::new(LangID::EN)
            .manufacturer("Raspberry Pi")
            .product("Pico 2 REPL")
            .serial_number("TEST")])
        .unwrap()
        .device_class(usbd_serial::USB_CLASS_CDC)
        .build();

    // Create our terminal and line editor
    let mut terminal = UsbCdcTerminal::new(usb_dev, serial);
    let mut editor = LineEditor::new(512, 50);  // 512 byte buffer, 50 history entries

    // Wait for terminal connection (DTR signal from picocom/minicom)
    terminal.wait_for_connection(&mut timer);

    // Send banner now that terminal is connected
    terminal.write(b"\r\n\r\nRaspberry Pi Pico 2 (RP2350) USB REPL with editline!\r\n").ok();
    terminal.write(b"Features: full line editing, history, word navigation\r\n").ok();
    terminal.write(b"Commands:\r\n").ok();
    terminal.write(b"  exit - Exit the REPL\r\n").ok();
    terminal.write(b"  help - Show this help message\r\n").ok();
    terminal.write(b"\r\n").ok();
    terminal.flush().ok();

    loop {
        terminal.write(b"pico2> ").ok();

        match editor.read_line(&mut terminal) {
            Ok(line) => {
                if line == "exit" {
                    terminal.write(b"Goodbye!\r\n").ok();
                    break;
                } else if line == "help" {
                    terminal.write(b"Available commands:\r\n").ok();
                    terminal.write(b"  exit - Exit the REPL\r\n").ok();
                    terminal.write(b"  help - Show this help message\r\n").ok();
                    terminal.write(b"\r\nKey bindings:\r\n").ok();
                    terminal.write(b"  Arrow keys: Navigate cursor and history\r\n").ok();
                    terminal.write(b"  Ctrl+Left/Right: Move by word\r\n").ok();
                    terminal.write(b"  Alt+Backspace: Delete word left\r\n").ok();
                    terminal.write(b"  Ctrl+Delete: Delete word right\r\n").ok();
                    terminal.write(b"  Home/End: Jump to start/end of line\r\n").ok();
                } else if !line.is_empty() {
                    terminal.write(b"You typed: ").ok();
                    terminal.write(line.as_bytes()).ok();
                    terminal.write(b"\r\n").ok();
                }
            }
            Err(_) => {
                terminal.write(b"\r\nError reading line\r\n").ok();
            }
        }
    }

    // Infinite loop after exit
    loop {
        cortex_m::asm::wfi();
    }
}
