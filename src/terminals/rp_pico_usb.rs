//! Raspberry Pi Pico USB CDC terminal implementation.
//!
//! This implementation provides a [`Terminal`](crate::Terminal) for the Raspberry Pi Pico
//! using USB CDC (Communications Device Class) for serial communication over the main USB port.
//!
//! # Examples
//!
//! ```no_run
//! use editline::terminals::rp_pico_usb::UsbCdcTerminal;
//!
//! // Assuming you have configured USB...
//! let terminal = UsbCdcTerminal::new(usb_device, serial_port);
//! ```

use usb_device::prelude::*;
use usbd_serial::SerialPort;
use crate::{Terminal, KeyEvent, Result, Error};

/// USB CDC terminal implementation for Raspberry Pi Pico.
///
/// Provides serial communication over USB CDC with support for ANSI escape
/// sequences (arrow keys, cursor control). The USB device appears as a
/// virtual COM port on the host computer.
///
/// # Type Parameters
///
/// * `B` - The USB bus type (typically `rp2040_hal::usb::UsbBus`)
pub struct UsbCdcTerminal<'a, B: usb_device::bus::UsbBus> {
    usb_device: UsbDevice<'a, B>,
    serial_port: SerialPort<'a, B>,
    read_buffer: [u8; 64],
    read_pos: usize,
    read_len: usize,
}

impl<'a, B: usb_device::bus::UsbBus> UsbCdcTerminal<'a, B> {
    /// Creates a new USB CDC terminal.
    ///
    /// # Arguments
    ///
    /// * `usb_device` - The configured USB device
    /// * `serial_port` - The USB CDC serial port
    pub fn new(usb_device: UsbDevice<'a, B>, serial_port: SerialPort<'a, B>) -> Self {
        Self {
            usb_device,
            serial_port,
            read_buffer: [0u8; 64],
            read_pos: 0,
            read_len: 0,
        }
    }

    /// Polls the USB device and reads available data into the internal buffer.
    fn poll_usb(&mut self) {
        if self.usb_device.poll(&mut [&mut self.serial_port]) {
            // Try to read into buffer if we've consumed all previous data
            if self.read_pos >= self.read_len {
                match self.serial_port.read(&mut self.read_buffer) {
                    Ok(count) if count > 0 => {
                        self.read_len = count;
                        self.read_pos = 0;
                    }
                    _ => {}
                }
            }
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

            // Otherwise poll USB until we get data
            self.poll_usb();
        }
    }

    /// Waits for USB to be configured and ready.
    ///
    /// This method blocks until the USB device reaches the `Configured` state.
    /// Note: This happens during USB enumeration when the device is plugged in,
    /// NOT when a terminal program connects to it.
    pub fn wait_until_configured(&mut self) {
        // Wait for USB to be configured
        loop {
            if self.usb_device.poll(&mut [&mut self.serial_port]) {
                if self.usb_device.state() == UsbDeviceState::Configured {
                    break;
                }
            }
        }
    }
}

impl<'a, B: usb_device::bus::UsbBus> Terminal for UsbCdcTerminal<'a, B> {
    fn read_byte(&mut self) -> Result<u8> {
        self.read_byte_blocking()
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        let mut written = 0;
        while written < data.len() {
            // Poll USB to keep it responsive
            self.poll_usb();

            // Try to write remaining data
            match self.serial_port.write(&data[written..]) {
                Ok(count) => {
                    written += count;
                }
                Err(UsbError::WouldBlock) => {
                    // Buffer full, keep polling until space available
                    continue;
                }
                Err(_) => {
                    return Err(Error::Io("USB write failed"));
                }
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        let _ = self.serial_port.flush();
        // Poll USB several times to ensure data is transmitted
        for _ in 0..10 {
            self.poll_usb();
        }
        Ok(())
    }

    fn enter_raw_mode(&mut self) -> Result<()> {
        // USB CDC is always in "raw" mode
        Ok(())
    }

    fn exit_raw_mode(&mut self) -> Result<()> {
        // USB CDC is always in "raw" mode
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
            // We need to poll until we get the next byte
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
