//! Windows terminal implementation using the Console API.
//!
//! This implementation uses Windows Console API functions to enable raw mode
//! (disabling line input and echo) and control the cursor position directly.

use crate::{KeyEvent, Terminal};
use std::io::{self, Write};
use winapi::um::consoleapi::{GetConsoleMode, SetConsoleMode};
use winapi::um::fileapi::ReadFile;
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::processenv::GetStdHandle;
use winapi::um::winbase::{STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
use winapi::um::wincon::{
    FillConsoleOutputCharacterA, GetConsoleScreenBufferInfo, SetConsoleCursorPosition,
    CONSOLE_SCREEN_BUFFER_INFO, COORD, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT,
    ENABLE_PROCESSED_INPUT,
};
use winapi::um::winnt::HANDLE;

/// Windows terminal using stdin/stdout with Console API.
///
/// Provides a [`Terminal`](crate::Terminal) implementation for Windows
/// using the native Console API for raw mode and cursor control.
///
/// # Examples
///
/// ```no_run
/// use editline::terminals::StdioTerminal;
///
/// let terminal = StdioTerminal::new().expect("Failed to create terminal");
/// ```
pub struct StdioTerminal {
    stdin_handle: HANDLE,
    stdout_handle: HANDLE,
    original_mode: Option<u32>,
}

impl StdioTerminal {
    /// Creates a new Windows terminal using stdin/stdout handles.
    ///
    /// # Errors
    ///
    /// Returns an error if the standard handles cannot be obtained.
    pub fn new() -> crate::Result<Self> {
        unsafe {
            let stdin_handle = GetStdHandle(STD_INPUT_HANDLE);
            let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);

            if stdin_handle == INVALID_HANDLE_VALUE || stdout_handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }

            Ok(Self {
                stdin_handle,
                stdout_handle,
                original_mode: None,
            })
        }
    }
}

impl Default for StdioTerminal {
    fn default() -> Self {
        Self::new().expect("Failed to initialize Windows terminal")
    }
}

impl Terminal for StdioTerminal {
    fn read_byte(&mut self) -> crate::Result<u8> {
        let mut buf = [0u8; 1];
        let mut bytes_read: u32 = 0;

        unsafe {
            if ReadFile(
                self.stdin_handle,
                buf.as_mut_ptr() as *mut _,
                1,
                &mut bytes_read,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
        }

        if bytes_read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF"));
        }

        Ok(buf[0])
    }

    fn write(&mut self, data: &[u8]) -> crate::Result<()> {
        let mut stdout = io::stdout();
        stdout.write_all(data)
    }

    fn flush(&mut self) -> crate::Result<()> {
        io::stdout().flush()
    }

    fn enter_raw_mode(&mut self) -> crate::Result<()> {
        unsafe {
            let mut mode: u32 = 0;
            if GetConsoleMode(self.stdin_handle, &mut mode) == 0 {
                return Err(io::Error::last_os_error());
            }

            self.original_mode = Some(mode);

            // Disable line input and echo
            let new_mode = mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT);

            if SetConsoleMode(self.stdin_handle, new_mode) == 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(())
    }

    fn exit_raw_mode(&mut self) -> crate::Result<()> {
        if let Some(original) = self.original_mode {
            unsafe {
                if SetConsoleMode(self.stdin_handle, original) == 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            self.original_mode = None;
        }

        Ok(())
    }

    fn cursor_left(&mut self) -> crate::Result<()> {
        self.write(b"\x08")
    }

    fn cursor_right(&mut self) -> crate::Result<()> {
        unsafe {
            let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(self.stdout_handle, &mut csbi) == 0 {
                return Err(io::Error::last_os_error());
            }

            let mut coord = csbi.dwCursorPosition;
            coord.X += 1;

            if SetConsoleCursorPosition(self.stdout_handle, coord) == 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(())
    }

    fn clear_eol(&mut self) -> crate::Result<()> {
        unsafe {
            let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(self.stdout_handle, &mut csbi) == 0 {
                return Err(io::Error::last_os_error());
            }

            let coord = csbi.dwCursorPosition;
            let count = (csbi.dwSize.X - coord.X) as u32;
            let mut written: u32 = 0;

            if FillConsoleOutputCharacterA(
                self.stdout_handle,
                b' ' as i8,
                count,
                coord,
                &mut written,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }

            if SetConsoleCursorPosition(self.stdout_handle, coord) == 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(())
    }

    fn parse_key_event(&mut self) -> crate::Result<KeyEvent> {
        let c = self.read_byte()?;

        // Enter
        if c == b'\r' || c == b'\n' {
            return Ok(KeyEvent::Enter);
        }

        // Ctrl-D (EOT - End of Transmission)
        // Standard Unix convention: EOF signal, should exit REPL
        if c == 4 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "EOF (Ctrl-D)"
            ));
        }

        // Ctrl-C (ETX - End of Text / Interrupt)
        // Standard Unix convention: interrupt signal, should cancel current line
        if c == 3 {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Interrupted (Ctrl-C)"
            ));
        }

        // Backspace
        if c == 8 || c == 127 {
            return Ok(KeyEvent::Backspace);
        }

        // Extended key prefix (0xE0 or 224)
        if c == 224 || c == 0 {
            let c2 = self.read_byte()?;

            match c2 {
                72 => return Ok(KeyEvent::Up),
                80 => return Ok(KeyEvent::Down),
                75 => return Ok(KeyEvent::Left),
                77 => return Ok(KeyEvent::Right),
                71 => return Ok(KeyEvent::Home),
                79 => return Ok(KeyEvent::End),
                83 => return Ok(KeyEvent::Delete),
                115 => return Ok(KeyEvent::CtrlLeft),   // Ctrl+Left
                116 => return Ok(KeyEvent::CtrlRight),  // Ctrl+Right
                _ => {}
            }
        }

        // Normal printable character
        if c >= 32 && c < 127 {
            return Ok(KeyEvent::Normal(c as char));
        }

        // Unknown - ignore
        Ok(KeyEvent::Normal('\0'))
    }
}

impl Drop for StdioTerminal {
    fn drop(&mut self) {
        let _ = self.exit_raw_mode();
    }
}
