//! micro:bit v2 terminal implementation using UART.
//!
//! This implementation provides a [`Terminal`](crate::Terminal) for the micro:bit v2
//! development board, using the nRF52833's UARTE peripheral for serial communication
//! over USB at 115200 baud.
//!
//! # Examples
//!
//! ```no_run
//! use editline::terminals::microbit::{from_board, Board};
//!
//! let board = Board::take().unwrap();
//! let terminal = from_board(board);
//! ```

use core::ptr::addr_of_mut;
use core::fmt::Write as FmtWrite;
use core::result::Result::{Ok, Err};
use embedded_io::Read as EmbeddedRead;
pub use microbit::{Board, hal::uarte::{Baudrate, Parity, Uarte, UarteRx, UarteTx, Instance}};
use crate::{Terminal, KeyEvent, Result, Error};

/// Transmit buffer for UART operations.
///
/// Single-byte buffer used for non-blocking UART transmission.
static mut TX_BUF: [u8; 1] = [0; 1];

/// Receive buffer for UART operations.
///
/// Single-byte buffer used for UART reception.
static mut RX_BUF: [u8; 1] = [0; 1];

/// UART terminal implementation for micro:bit v2.
///
/// Provides serial communication at 115200 baud with support for ANSI escape
/// sequences (arrow keys, cursor control). Designed for use with serial terminal
/// programs like minicom, screen, or PuTTY.
///
/// # Type Parameters
///
/// * `T` - The UARTE instance type (typically `microbit::pac::UARTE0`)
pub struct UarteTerminal<T: Instance> {
    tx: UarteTx<T>,
    rx: UarteRx<T>,
}

impl<T: Instance> UarteTerminal<T> {
    /// Creates a new UART terminal from a UARTE peripheral.
    ///
    /// Splits the UARTE into separate transmit and receive halves using
    /// the static TX_BUF and RX_BUF buffers.
    ///
    /// # Arguments
    ///
    /// * `serial` - A configured UARTE peripheral
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use microbit::{Board, hal::uarte::{Baudrate, Parity, Uarte}};
    /// use editline::terminals::microbit::UarteTerminal;
    ///
    /// let board = Board::take().unwrap();
    /// let serial = Uarte::new(
    ///     board.UARTE0,
    ///     board.uart.into(),
    ///     Parity::EXCLUDED,
    ///     Baudrate::BAUD115200,
    /// );
    /// let terminal = UarteTerminal::new(serial);
    /// ```
    pub fn new(serial: Uarte<T>) -> Self {
        let (tx, rx) = serial
            .split(unsafe { addr_of_mut!(TX_BUF).as_mut().unwrap() }, unsafe {
                addr_of_mut!(RX_BUF).as_mut().unwrap()
            })
            .unwrap();
        Self { tx, rx }
    }

    /// Reads a single byte from UART, blocking until available.
    ///
    /// # Errors
    ///
    /// Returns an error if the UART read operation fails.
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

/// Creates a UART terminal from a micro:bit board.
///
/// Convenience function that configures the UARTE0 peripheral with standard
/// settings (115200 baud, no parity) and returns a ready-to-use terminal.
///
/// # Arguments
///
/// * `board` - The micro:bit board obtained from [`Board::take()`]
///
/// # Examples
///
/// ```no_run
/// use editline::terminals::microbit::{from_board, Board};
/// use editline::LineEditor;
///
/// let board = Board::take().unwrap();
/// let mut terminal = from_board(board);
/// let mut editor = LineEditor::new(256, 20);
///
/// loop {
///     match editor.read_line(&mut terminal) {
///         Ok(line) => { /* process line */ }
///         Err(_) => break,
///     }
/// }
/// ```
pub fn from_board(board: Board) -> UarteTerminal<microbit::pac::UARTE0> {
    let serial = Uarte::new(
        board.UARTE0,
        board.uart.into(),
        Parity::EXCLUDED,
        Baudrate::BAUD115200,
    );
    UarteTerminal::new(serial)
}
