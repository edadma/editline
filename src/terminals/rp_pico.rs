//! Raspberry Pi Pico terminal implementation using UART.
//!
//! This implementation provides a [`Terminal`](crate::Terminal) for the Raspberry Pi Pico
//! development board, using the RP2040's UART0 peripheral for serial communication
//! over USB at 115200 baud.
//!
//! # Examples
//!
//! ```no_run
//! use editline::terminals::rp_pico::UartTerminal;
//! use rp2040_hal::{uart::{UartPeripheral, DataBits, StopBits}, gpio::Pins};
//!
//! // Assuming you have configured pac, pins, and uart0...
//! let terminal = UartTerminal::new(uart0);
//! ```

use embedded_io::{Read as EmbeddedRead, Write as EmbeddedWrite};
pub use rp2040_hal::uart::{UartPeripheral, DataBits, StopBits, Enabled, UartDevice, ValidUartPinout};
use crate::{Terminal, KeyEvent, Result, Error};

/// UART terminal implementation for Raspberry Pi Pico.
///
/// Provides serial communication at 115200 baud with support for ANSI escape
/// sequences (arrow keys, cursor control). Designed for use with serial terminal
/// programs like minicom, screen, or PuTTY.
///
/// # Type Parameters
///
/// * `D` - The UART device type (typically `rp2040_hal::pac::UART0` or `UART1`)
/// * `P` - The pins type for TX/RX
pub struct UartTerminal<D: UartDevice, P: ValidUartPinout<D>> {
    uart: UartPeripheral<Enabled, D, P>,
}

impl<D: UartDevice, P: ValidUartPinout<D>> UartTerminal<D, P> {
    /// Creates a new UART terminal from a configured UART peripheral.
    ///
    /// # Arguments
    ///
    /// * `uart` - A configured UART peripheral
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rp2040_hal::{uart::{UartPeripheral, DataBits, StopBits}, clocks::ClocksManager};
    /// use editline::terminals::rp_pico::UartTerminal;
    ///
    /// // Assuming pac, pins, and clocks are set up...
    /// let uart = UartPeripheral::new(pac.UART0, pins, &mut pac.RESETS)
    ///     .enable(
    ///         uart::common_configs::_115200_8_N_1,
    ///         clocks.peripheral_clock.freq(),
    ///     ).unwrap();
    /// let terminal = UartTerminal::new(uart);
    /// ```
    pub fn new(uart: UartPeripheral<Enabled, D, P>) -> Self {
        Self { uart }
    }

    /// Reads a single byte from UART, blocking until available.
    ///
    /// # Errors
    ///
    /// Returns an error if the UART read operation fails.
    fn read_byte_blocking(&mut self) -> Result<u8> {
        let mut buf = [0u8];
        self.uart.read_exact(&mut buf).map_err(|_| Error::Io("UART read failed"))?;
        Ok(buf[0])
    }
}

impl<D: UartDevice, P: ValidUartPinout<D>> Terminal for UartTerminal<D, P> {
    fn read_byte(&mut self) -> Result<u8> {
        self.read_byte_blocking()
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.uart.write_all(data)
            .map_err(|_| Error::Io("UART write failed"))
    }

    fn flush(&mut self) -> Result<()> {
        self.uart.flush()
            .map_err(|_| Error::Io("UART flush failed"))
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
            if self.uart.read(&mut buf).is_ok() {
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
                                    // Ctrl+Delete is ESC[3;5~
                                    if c4 == b';' {
                                        if let Ok(c5) = self.read_byte_blocking() {
                                            if c5 == b'5' {
                                                if let Ok(c6) = self.read_byte_blocking() {
                                                    if c6 == b'~' {
                                                        return Ok(KeyEvent::CtrlDelete);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Extended sequences like ESC[1;5D (Ctrl+Left)
                            b'1' => {
                                if let Ok(semicolon) = self.read_byte_blocking() {
                                    if semicolon == b';' {
                                        if let Ok(modifier) = self.read_byte_blocking() {
                                            if modifier == b'5' { // Ctrl modifier
                                                if let Ok(final_byte) = self.read_byte_blocking() {
                                                    match final_byte {
                                                        b'D' => return Ok(KeyEvent::CtrlLeft),
                                                        b'C' => return Ok(KeyEvent::CtrlRight),
                                                        _ => {} // Unknown Ctrl+key combo, drain
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                // If we get here, drain the rest of the sequence
                                return Ok(KeyEvent::Normal('\0'));
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
