use core::ptr::addr_of_mut;
use core::fmt::Write as FmtWrite;
use embedded_io::Read as EmbeddedRead;
pub use microbit::{Board, hal::uarte::{Baudrate, Parity, Uarte, UarteRx, UarteTx, Instance}};
use crate::{Terminal, KeyEvent, Result, Error};

static mut TX_BUF: [u8; 1] = [0; 1];
static mut RX_BUF: [u8; 1] = [0; 1];

/// UART Terminal implementation for micro:bit
pub struct UarteTerminal<T: Instance> {
    tx: UarteTx<T>,
    rx: UarteRx<T>,
}

impl<T: Instance> UarteTerminal<T> {
    pub fn new(serial: Uarte<T>) -> Self {
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

/// Helper function to create a UarteTerminal from the micro:bit board
pub fn from_board(board: Board) -> UarteTerminal<microbit::pac::UARTE0> {
    let serial = Uarte::new(
        board.UARTE0,
        board.uart.into(),
        Parity::EXCLUDED,
        Baudrate::BAUD115200,
    );
    UarteTerminal::new(serial)
}
