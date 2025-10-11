// Unix terminal implementation using termios and ANSI escape codes

use crate::{KeyEvent, Terminal};
use std::io::{self, Read, Write};
use std::os::unix::io::AsRawFd;

/// Unix terminal using stdin/stdout with termios
pub struct StdioTerminal {
    stdin: io::Stdin,
    stdout: io::Stdout,
    original_termios: Option<libc::termios>,
}

impl StdioTerminal {
    pub fn new() -> Self {
        Self {
            stdin: io::stdin(),
            stdout: io::stdout(),
            original_termios: None,
        }
    }

    fn read_byte_internal(&mut self) -> io::Result<u8> {
        let mut buf = [0u8; 1];
        self.stdin.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

impl Default for StdioTerminal {
    fn default() -> Self {
        Self::new()
    }
}

impl Terminal for StdioTerminal {
    fn read_byte(&mut self) -> io::Result<u8> {
        self.read_byte_internal()
    }

    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.stdout.write_all(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }

    fn enter_raw_mode(&mut self) -> io::Result<()> {
        let fd = self.stdin.as_raw_fd();

        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();

            if libc::tcgetattr(fd, &mut termios) != 0 {
                return Err(io::Error::last_os_error());
            }

            // Save original settings
            self.original_termios = Some(termios);

            // Disable canonical mode and echo
            termios.c_lflag &= !(libc::ECHO | libc::ICANON);

            // Set minimum characters and timeout
            termios.c_cc[libc::VMIN] = 1;
            termios.c_cc[libc::VTIME] = 0;

            if libc::tcsetattr(fd, libc::TCSAFLUSH, &termios) != 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(())
    }

    fn exit_raw_mode(&mut self) -> io::Result<()> {
        if let Some(original) = self.original_termios {
            let fd = self.stdin.as_raw_fd();

            unsafe {
                if libc::tcsetattr(fd, libc::TCSAFLUSH, &original) != 0 {
                    return Err(io::Error::last_os_error());
                }
            }

            self.original_termios = None;
        }

        Ok(())
    }

    fn cursor_left(&mut self) -> io::Result<()> {
        self.write(b"\x1b[D")
    }

    fn cursor_right(&mut self) -> io::Result<()> {
        self.write(b"\x1b[C")
    }

    fn clear_eol(&mut self) -> io::Result<()> {
        self.write(b"\x1b[K")
    }

    fn parse_key_event(&mut self) -> io::Result<KeyEvent> {
        let c = self.read_byte_internal()?;

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
            // Read next byte
            let c2 = self.read_byte_internal()?;

            // Alt+Backspace
            if c2 == 127 || c2 == 8 {
                return Ok(KeyEvent::AltBackspace);
            }

            // ESC[ sequences (ANSI)
            if c2 == b'[' {
                let c3 = self.read_byte_internal()?;

                match c3 {
                    b'A' => return Ok(KeyEvent::Up),
                    b'B' => return Ok(KeyEvent::Down),
                    b'C' => return Ok(KeyEvent::Right),
                    b'D' => return Ok(KeyEvent::Left),
                    b'H' => return Ok(KeyEvent::Home),
                    b'F' => return Ok(KeyEvent::End),
                    b'1' => {
                        let c4 = self.read_byte_internal()?;
                        if c4 == b'~' {
                            return Ok(KeyEvent::Home);
                        } else if c4 == b';' {
                            // Ctrl+key sequences
                            let c5 = self.read_byte_internal()?;
                            if c5 == b'5' {
                                let c6 = self.read_byte_internal()?;
                                match c6 {
                                    b'C' => return Ok(KeyEvent::CtrlRight),
                                    b'D' => return Ok(KeyEvent::CtrlLeft),
                                    _ => {}
                                }
                            }
                        }
                    }
                    b'3' => {
                        let c4 = self.read_byte_internal()?;
                        if c4 == b'~' {
                            return Ok(KeyEvent::Delete);
                        } else if c4 == b';' {
                            let c5 = self.read_byte_internal()?;
                            if c5 == b'5' {
                                let c6 = self.read_byte_internal()?;
                                if c6 == b'~' {
                                    return Ok(KeyEvent::CtrlDelete);
                                }
                            }
                        }
                    }
                    b'4' => {
                        let c4 = self.read_byte_internal()?;
                        if c4 == b'~' {
                            return Ok(KeyEvent::End);
                        }
                    }
                    _ => {}
                }
            }

            // Unknown escape sequence - treat as normal char
            if (32..127).contains(&c2) {
                if let Ok(ch) = std::str::from_utf8(&[c2]) {
                    if let Some(ch) = ch.chars().next() {
                        return Ok(KeyEvent::Normal(ch));
                    }
                }
            }
        }

        // Normal printable character
        if (32..127).contains(&c) {
            return Ok(KeyEvent::Normal(c as char));
        }

        // Unknown/control character - ignore
        Ok(KeyEvent::Normal('\0'))
    }
}

impl Drop for StdioTerminal {
    fn drop(&mut self) {
        let _ = self.exit_raw_mode();
    }
}
