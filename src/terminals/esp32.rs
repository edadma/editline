//! ESP32-S3 USB Serial/JTAG terminal implementation.
//!
//! This implementation provides a [`Terminal`](crate::Terminal) for the ESP32-S3
//! using USB Serial/JTAG for serial communication over the built-in USB port.
//!
//! # Examples
//!
//! ```no_run
//! use editline::terminals::esp32::UsbSerialJtagTerminal;
//!
//! let mut terminal = UsbSerialJtagTerminal::new();
//! ```

use crate::{Terminal, KeyEvent, Result, Error};
use esp_idf_svc::sys::{
    usb_serial_jtag_read_bytes,
    usb_serial_jtag_write_bytes,
};
use std::ffi::c_void;

/// USB Serial/JTAG terminal implementation for ESP32-S3.
///
/// Provides serial communication over the built-in USB Serial/JTAG interface
/// with support for ANSI escape sequences (arrow keys, cursor control).
///
/// The driver must be initialized before creating this terminal using
/// `usb_serial_jtag_driver_install`.
pub struct UsbSerialJtagTerminal {
    read_buffer: [u8; 64],
    read_pos: usize,
    read_len: usize,
}

impl UsbSerialJtagTerminal {
    /// Creates a new USB Serial/JTAG terminal.
    ///
    /// # Safety
    ///
    /// The USB Serial/JTAG driver must be installed before calling this
    /// using `usb_serial_jtag_driver_install`.
    pub fn new() -> Self {
        Self {
            read_buffer: [0u8; 64],
            read_pos: 0,
            read_len: 0,
        }
    }

    /// Reads a single byte from the USB serial port, blocking until available.
    fn read_byte_blocking(&mut self) -> Result<u8> {
        loop {
            // If we have buffered data, return it
            if self.read_pos < self.read_len {
                let byte = self.read_buffer[self.read_pos];
                self.read_pos += 1;
                return Ok(byte);
            }

            // Try to read more data (non-blocking with timeout 0)
            let bytes_read = unsafe {
                usb_serial_jtag_read_bytes(
                    self.read_buffer.as_mut_ptr() as *mut c_void,
                    self.read_buffer.len() as u32,
                    0, // No timeout - return immediately
                )
            };

            if bytes_read > 0 {
                self.read_len = bytes_read as usize;
                self.read_pos = 0;
            } else {
                // Yield to FreeRTOS scheduler to avoid busy-waiting
                unsafe {
                    esp_idf_svc::sys::vTaskDelay(1);
                }
            }
        }
    }
}

impl Default for UsbSerialJtagTerminal {
    fn default() -> Self {
        Self::new()
    }
}

impl Terminal for UsbSerialJtagTerminal {
    fn read_byte(&mut self) -> Result<u8> {
        self.read_byte_blocking()
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        let mut written = 0;
        while written < data.len() {
            let chunk = &data[written..];
            let bytes_written = unsafe {
                usb_serial_jtag_write_bytes(
                    chunk.as_ptr() as *const c_void,
                    chunk.len(),
                    100, // 100ms timeout
                )
            };

            if bytes_written > 0 {
                written += bytes_written as usize;
            } else if bytes_written < 0 {
                return Err(Error::Io("USB write failed"));
            }
            // If bytes_written == 0, retry (timeout occurred)
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        // ESP-IDF's usb_serial_jtag_write_bytes is already blocking until data is transmitted
        Ok(())
    }

    fn enter_raw_mode(&mut self) -> Result<()> {
        // USB Serial/JTAG is always in "raw" mode
        Ok(())
    }

    fn exit_raw_mode(&mut self) -> Result<()> {
        // USB Serial/JTAG is always in "raw" mode
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
            // Try to read next byte for escape sequence
            let c2 = self.read_byte_blocking()?;

            // Alt+Backspace
            if c2 == 127 || c2 == 8 {
                return Ok(KeyEvent::AltBackspace);
            }

            // ESC[ sequences (ANSI)
            if c2 == b'[' {
                let c3 = self.read_byte_blocking()?;
                match c3 {
                    b'A' => return Ok(KeyEvent::Up),
                    b'B' => return Ok(KeyEvent::Down),
                    b'C' => return Ok(KeyEvent::Right),
                    b'D' => return Ok(KeyEvent::Left),
                    b'H' => return Ok(KeyEvent::Home),
                    b'F' => return Ok(KeyEvent::End),
                    b'3' => {
                        let c4 = self.read_byte_blocking()?;
                        if c4 == b'~' {
                            return Ok(KeyEvent::Delete);
                        }
                        // Ctrl+Delete is ESC[3;5~
                        if c4 == b';' {
                            let c5 = self.read_byte_blocking()?;
                            if c5 == b'5' {
                                let c6 = self.read_byte_blocking()?;
                                if c6 == b'~' {
                                    return Ok(KeyEvent::CtrlDelete);
                                }
                            }
                        }
                    }
                    // Extended sequences like ESC[1;5D (Ctrl+Left)
                    b'1' => {
                        let semicolon = self.read_byte_blocking()?;
                        if semicolon == b';' {
                            let modifier = self.read_byte_blocking()?;
                            if modifier == b'5' {
                                // Ctrl modifier
                                let final_byte = self.read_byte_blocking()?;
                                match final_byte {
                                    b'D' => return Ok(KeyEvent::CtrlLeft),
                                    b'C' => return Ok(KeyEvent::CtrlRight),
                                    _ => {} // Unknown Ctrl+key combo
                                }
                            }
                        }
                        // Drain rest of sequence
                        return Ok(KeyEvent::Normal('\0'));
                    }
                    // Unknown escape sequence - consume until we hit a letter or tilde
                    _ => {
                        let mut byte = c3;
                        // Drain sequence: read until we get a letter (A-Z, a-z) or tilde
                        while !byte.is_ascii_alphabetic() && byte != b'~' {
                            byte = self.read_byte_blocking()?;
                        }
                        // Return null to ignore this unknown sequence
                        return Ok(KeyEvent::Normal('\0'));
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
