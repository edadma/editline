//! micro:bit REPL example using editline
//!
//! To build and run this example:
//! ```
//! cargo build --example microbit_repl --no-default-features
//! cargo run --example microbit_repl --no-default-features
//! ```
//!
//! The `--no-default-features` flag is required to disable the `std` feature
//! for embedded targets.

#![no_std]
#![no_main]

extern crate alloc;

use cortex_m_rt::entry;
use panic_halt as _;
use core::ptr::addr_of_mut;
use core::fmt::Write as FmtWrite;
use embedded_io::Read as EmbeddedRead;
use microbit::{Board, hal::uarte::{Baudrate, Parity, Uarte, UarteRx, UarteTx, Instance}};
use editline::{LineEditor, Terminal, KeyEvent, Result, Error};
use alloc_cortex_m::CortexMHeap;

static mut TX_BUF: [u8; 1] = [0; 1];
static mut RX_BUF: [u8; 1] = [0; 1];

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

/// UART Terminal implementation for micro:bit
struct UarteTerminal<T: Instance> {
    tx: UarteTx<T>,
    rx: UarteRx<T>,
}

impl<T: Instance> UarteTerminal<T> {
    fn new(serial: Uarte<T>) -> Self {
        let (tx, rx) = serial
            .split(unsafe { addr_of_mut!(TX_BUF).as_mut().unwrap() }, unsafe {
                addr_of_mut!(RX_BUF).as_mut().unwrap()
            })
            .unwrap();
        Self { tx, rx }
    }

    fn read_byte_blocking(&mut self) -> Result<u8> {
        let mut buf = [0u8];
        self.rx.read_exact(&mut buf).map_err(|_| Error::Io("UART read failed"))?;
        Ok(buf[0])
    }
}

impl<T: Instance> Terminal for UarteTerminal<T> {
    fn read_byte(&mut self) -> Result<u8> {
        self.read_byte_blocking()
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.tx.write_str(core::str::from_utf8(data).map_err(|_| Error::InvalidUtf8)?)
            .map_err(|_| Error::Io("UART write failed"))
    }

    fn flush(&mut self) -> Result<()> {
        // UART on micro:bit doesn't buffer, so flush is a no-op
        Ok(())
    }

    fn enter_raw_mode(&mut self) -> Result<()> {
        // UART is always in "raw" mode
        Ok(())
    }

    fn exit_raw_mode(&mut self) -> Result<()> {
        // UART is always in "raw" mode
        Ok(())
    }

    fn cursor_left(&mut self) -> Result<()> {
        self.write(b"\x1b[D")
    }

    fn cursor_right(&mut self) -> Result<()> {
        self.write(b"\x1b[C")
    }

    fn clear_eol(&mut self) -> Result<()> {
        self.write(b"\x1b[K")
    }

    fn parse_key_event(&mut self) -> Result<KeyEvent> {
        let c = self.read_byte_blocking()?;

        // Enter/Return
        if c == b'\r' || c == b'\n' {
            return Ok(KeyEvent::Enter);
        }

        // Backspace
        if c == 127 || c == 8 {
            return Ok(KeyEvent::Backspace);
        }

        // ESC sequences
        if c == 27 {
            // Try to read next byte for escape sequence (non-blocking)
            let mut buf = [0u8];
            if self.rx.read(&mut buf).is_ok() {
                let c2 = buf[0];

                // Alt+Backspace
                if c2 == 127 || c2 == 8 {
                    return Ok(KeyEvent::AltBackspace);
                }

                // ESC[ sequences (ANSI)
                if c2 == b'[' {
                    if let Ok(c3) = self.read_byte_blocking() {
                        match c3 {
                            b'A' => return Ok(KeyEvent::Up),
                            b'B' => return Ok(KeyEvent::Down),
                            b'C' => return Ok(KeyEvent::Right),
                            b'D' => return Ok(KeyEvent::Left),
                            b'H' => return Ok(KeyEvent::Home),
                            b'F' => return Ok(KeyEvent::End),
                            b'3' => {
                                if let Ok(c4) = self.read_byte_blocking() {
                                    if c4 == b'~' {
                                        return Ok(KeyEvent::Delete);
                                    }
                                }
                            }
                            // Unknown escape sequence - consume until we hit a letter or tilde
                            _ => {
                                let mut byte = c3;
                                // Drain sequence: read until we get a letter (A-Z, a-z) or tilde
                                while !byte.is_ascii_alphabetic() && byte != b'~' {
                                    if let Ok(b) = self.read_byte_blocking() {
                                        byte = b;
                                    } else {
                                        break;
                                    }
                                }
                                // Return null to ignore this unknown sequence
                                return Ok(KeyEvent::Normal('\0'));
                            }
                        }
                    }
                }
            }
            // If we got ESC but couldn't parse a valid sequence, ignore it
            return Ok(KeyEvent::Normal('\0'));
        }

        // Normal printable character
        if (32..127).contains(&c) {
            return Ok(KeyEvent::Normal(c as char));
        }

        // Unknown/control character - treat as null
        Ok(KeyEvent::Normal('\0'))
    }
}

#[entry]
fn main() -> ! {
    // Initialize the allocator
    const HEAP_SIZE: usize = 4096;
    static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    unsafe { ALLOCATOR.init(&raw mut HEAP as *const u8 as usize, HEAP_SIZE) }

    let board = Board::take().unwrap();

    let serial = Uarte::new(
        board.UARTE0,
        board.uart.into(),
        Parity::EXCLUDED,
        Baudrate::BAUD115200,
    );

    let mut terminal = UarteTerminal::new(serial);
    let mut editor = LineEditor::new(256, 20);  // 256 byte buffer, 20 history entries

    terminal.write(b"micro:bit Rust REPL with editline!\r\n").ok();
    terminal.write(b"Features: history (up/down), line editing, backspace\r\n\r\n").ok();

    loop {
        terminal.write(b"> ").ok();

        match editor.read_line(&mut terminal) {
            Ok(line) => {
                terminal.write(b"You typed: ").ok();
                terminal.write(line.as_bytes()).ok();
                terminal.write(b"\r\n").ok();
            }
            Err(_) => {
                terminal.write(b"\r\nError reading line\r\n").ok();
            }
        }
    }
}
