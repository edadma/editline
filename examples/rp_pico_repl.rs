//! Raspberry Pi Pico REPL example using editline
//!
//! To build and flash this example:
//! ```
//! cargo build --example rp_pico_repl --target thumbv6m-none-eabi --no-default-features --features rp_pico --release
//! elf2uf2-rs target/thumbv6m-none-eabi/release/examples/rp_pico_repl
//! ```
//!
//! Then copy the .uf2 file to your Pico in BOOTSEL mode.
//!
//! Connect to the Pico's serial port (typically /dev/ttyACM0 or COM3) at 115200 baud:
//! ```
//! minicom -D /dev/ttyACM0 -b 115200
//! ```

#![no_std]
#![no_main]

extern crate alloc;

use cortex_m_rt::entry;
use panic_halt as _;
use alloc_cortex_m::CortexMHeap;

use rp2040_hal::{
    clocks::{init_clocks_and_plls, Clock},
    pac,
    uart::{DataBits, StopBits, UartConfig, UartPeripheral},
    watchdog::Watchdog,
    Sio,
};

use editline::{LineEditor, Terminal, terminals::rp_pico::UartTerminal};
use fugit::RateExtU32;

// Link boot stage 2
#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

// External high-speed crystal on the Pico board is 12MHz
const XOSC_CRYSTAL_FREQ: u32 = 12_000_000u32;

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

#[entry]
fn main() -> ! {
    // Initialize the allocator
    const HEAP_SIZE: usize = 8192;
    static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    unsafe { ALLOCATOR.init(&raw mut HEAP as *const u8 as usize, HEAP_SIZE) }

    // Grab singleton objects
    let mut pac = pac::Peripherals::take().unwrap();
    let _core = pac::CorePeripherals::take().unwrap();

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

    // Set up the GPIO pins
    let sio = Sio::new(pac.SIO);
    let pins = rp2040_hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Configure UART0 on pins GP0 (TX) and GP1 (RX)
    let uart_pins = (
        pins.gpio0.into_function::<rp2040_hal::gpio::FunctionUart>(),
        pins.gpio1.into_function::<rp2040_hal::gpio::FunctionUart>(),
    );

    let uart = UartPeripheral::new(pac.UART0, uart_pins, &mut pac.RESETS)
        .enable(
            UartConfig::new(
                115200.Hz(),
                DataBits::Eight,
                None,
                StopBits::One,
            ),
            clocks.peripheral_clock.freq(),
        )
        .unwrap();

    let mut terminal = UartTerminal::new(uart);
    let mut editor = LineEditor::new(512, 50);  // 512 byte buffer, 50 history entries

    terminal.write(b"\r\n\r\n").ok();
    terminal.write(b"Raspberry Pi Pico REPL with editline!\r\n").ok();
    terminal.write(b"Features: full line editing, history, word navigation\r\n").ok();
    terminal.write(b"Commands:\r\n").ok();
    terminal.write(b"  exit - Exit the REPL\r\n").ok();
    terminal.write(b"  help - Show this help message\r\n").ok();
    terminal.write(b"\r\n").ok();

    loop {
        terminal.write(b"pico> ").ok();

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
