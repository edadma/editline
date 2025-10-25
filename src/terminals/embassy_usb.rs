//! Embassy USB CDC terminal implementation for STM32 and other Embassy-supported microcontrollers.
//!
//! This module provides an async terminal implementation using Embassy's USB CDC (Communications
//! Device Class) driver. It's designed for embedded systems using the Embassy async runtime.

use crate::{AsyncTerminal, Error, KeyEvent, Result};
use embassy_usb::class::cdc_acm::CdcAcmClass;
use embassy_usb::driver::EndpointError;

/// Embassy USB CDC terminal for async line editing on embedded systems.
///
/// This terminal implementation wraps an Embassy USB CDC ACM class and provides
/// async I/O operations suitable for use with [`AsyncLineEditor`](crate::AsyncLineEditor).
///
/// # Example
///
/// ```ignore
/// use editline::{AsyncLineEditor, terminals::EmbassyUsbTerminal};
/// use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
///
/// // After setting up USB driver and CDC ACM class...
/// let mut terminal = EmbassyUsbTerminal::new(class);
/// let mut editor = AsyncLineEditor::new(1024, 50);
///
/// loop {
///     terminal.write(b"> ").await.unwrap();
///     terminal.flush().await.unwrap();
///
///     match editor.read_line(&mut terminal).await {
///         Ok(line) => {
///             if line == "exit" {
///                 break;
///             }
///             defmt::info!("Got: {}", line);
///         }
///         Err(e) => {
///             defmt::error!("Error: {:?}", e);
///             break;
///         }
///     }
/// }
/// ```
pub struct EmbassyUsbTerminal<'d, D: embassy_usb::driver::Driver<'d>> {
    class: CdcAcmClass<'d, D>,
    input_buffer: [u8; 64],
    input_pos: usize,
    input_len: usize,
}

impl<'d, D: embassy_usb::driver::Driver<'d>> EmbassyUsbTerminal<'d, D> {
    /// Creates a new Embassy USB CDC terminal.
    ///
    /// # Arguments
    ///
    /// * `class` - The Embassy USB CDC ACM class instance
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let terminal = EmbassyUsbTerminal::new(class);
    /// ```
    pub fn new(class: CdcAcmClass<'d, D>) -> Self {
        Self {
            class,
            input_buffer: [0; 64],
            input_pos: 0,
            input_len: 0,
        }
    }

    /// Checks if DTR (Data Terminal Ready) is active.
    ///
    /// Returns `true` if a terminal is connected and DTR is active.
    /// This is useful for detecting when a serial terminal connects.
    pub fn dtr(&self) -> bool {
        self.class.dtr()
    }

    /// Waits for the terminal to connect (DTR to become active).
    ///
    /// This is a convenience method that polls DTR status and yields
    /// control to the executor until DTR becomes active.
    pub async fn wait_connection(&mut self) {
        loop {
            if self.class.dtr() {
                // Wait a bit for terminal to be fully ready
                embassy_time::Timer::after_millis(100).await;
                break;
            }
            embassy_time::Timer::after_millis(20).await;
        }
    }

    /// Reads more data into the internal buffer if needed.
    async fn fill_buffer(&mut self) -> Result<()> {
        if self.input_pos >= self.input_len {
            // Buffer is empty, read more data
            loop {
                match self.class.read_packet(&mut self.input_buffer).await {
                    Ok(n) if n > 0 => {
                        self.input_len = n;
                        self.input_pos = 0;
                        return Ok(());
                    }
                    Ok(_) => {
                        // Zero-length read, try again
                        continue;
                    }
                    Err(EndpointError::Disabled) => {
                        return Err(Error::Eof);
                    }
                    Err(_) => {
                        // Transient error, try again
                        continue;
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'d, D: embassy_usb::driver::Driver<'d>> AsyncTerminal for EmbassyUsbTerminal<'d, D> {
    async fn read_byte(&mut self) -> Result<u8> {
        self.fill_buffer().await?;
        let byte = self.input_buffer[self.input_pos];
        self.input_pos += 1;
        Ok(byte)
    }

    async fn write(&mut self, data: &[u8]) -> Result<()> {
        // Split into chunks if necessary (USB CDC has max packet size)
        let mut pos = 0;
        while pos < data.len() {
            let chunk_size = core::cmp::min(data.len() - pos, 64);
            let chunk = &data[pos..pos + chunk_size];

            loop {
                match self.class.write_packet(chunk).await {
                    Ok(_) => break,
                    Err(EndpointError::Disabled) => {
                        return Err(Error::Eof);
                    }
                    Err(_) => {
                        // Transient error, retry
                        continue;
                    }
                }
            }

            pos += chunk_size;
        }
        Ok(())
    }

    async fn flush(&mut self) -> Result<()> {
        // USB CDC doesn't have an explicit flush operation
        // Writing is already immediate
        Ok(())
    }

    async fn enter_raw_mode(&mut self) -> Result<()> {
        // USB CDC is already in "raw" mode - no line buffering or echo
        Ok(())
    }

    async fn exit_raw_mode(&mut self) -> Result<()> {
        // Nothing to do
        Ok(())
    }

    async fn cursor_left(&mut self) -> Result<()> {
        self.write(b"\x1b[D").await
    }

    async fn cursor_right(&mut self) -> Result<()> {
        self.write(b"\x1b[C").await
    }

    async fn clear_eol(&mut self) -> Result<()> {
        self.write(b"\x1b[K").await
    }

    async fn parse_key_event(&mut self) -> Result<KeyEvent> {
        let b = self.read_byte().await?;

        match b {
            // Normal printable characters
            0x20..=0x7E => Ok(KeyEvent::Normal(b as char)),

            // Backspace (both BS and DEL)
            0x08 | 0x7F => Ok(KeyEvent::Backspace),

            // Enter (both CR and LF)
            b'\r' | b'\n' => Ok(KeyEvent::Enter),

            // Tab
            b'\t' => Ok(KeyEvent::Normal('\t')),

            // ESC - start of escape sequence
            0x1b => {
                let b2 = self.read_byte().await?;
                match b2 {
                    b'[' => {
                        // CSI sequence
                        let b3 = self.read_byte().await?;
                        match b3 {
                            b'A' => Ok(KeyEvent::Up),
                            b'B' => Ok(KeyEvent::Down),
                            b'C' => Ok(KeyEvent::Right),
                            b'D' => Ok(KeyEvent::Left),
                            b'H' => Ok(KeyEvent::Home),
                            b'F' => Ok(KeyEvent::End),
                            b'3' => {
                                // Delete key: ESC[3~
                                let b4 = self.read_byte().await?;
                                if b4 == b'~' {
                                    Ok(KeyEvent::Delete)
                                } else {
                                    // Unknown sequence, ignore
                                    Ok(KeyEvent::Normal(' '))
                                }
                            }
                            b'1' => {
                                // Could be Home (ESC[1~) or other sequences
                                let b4 = self.read_byte().await?;
                                match b4 {
                                    b'~' => Ok(KeyEvent::Home),
                                    b';' => {
                                        // Modifier sequence like ESC[1;5C (Ctrl+Right)
                                        let b5 = self.read_byte().await?;
                                        if b5 == b'5' {
                                            let b6 = self.read_byte().await?;
                                            match b6 {
                                                b'C' => Ok(KeyEvent::CtrlRight),
                                                b'D' => Ok(KeyEvent::CtrlLeft),
                                                _ => Ok(KeyEvent::Normal(' ')),
                                            }
                                        } else {
                                            Ok(KeyEvent::Normal(' '))
                                        }
                                    }
                                    _ => Ok(KeyEvent::Normal(' ')),
                                }
                            }
                            b'4' => {
                                // End key: ESC[4~
                                let b4 = self.read_byte().await?;
                                if b4 == b'~' {
                                    Ok(KeyEvent::End)
                                } else {
                                    Ok(KeyEvent::Normal(' '))
                                }
                            }
                            _ => {
                                // Unknown CSI sequence, ignore
                                Ok(KeyEvent::Normal(' '))
                            }
                        }
                    }
                    0x7F => {
                        // Alt+Backspace
                        Ok(KeyEvent::AltBackspace)
                    }
                    _ => {
                        // Unknown escape sequence, ignore
                        Ok(KeyEvent::Normal(' '))
                    }
                }
            }

            // Ctrl+C
            0x03 => Err(Error::Interrupted),

            // Ctrl+D
            0x04 => Err(Error::Eof),

            // Other control characters - ignore
            _ => Ok(KeyEvent::Normal(' ')),
        }
    }
}
