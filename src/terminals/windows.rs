//! Windows terminal implementation using the Console API.
//!
//! This implementation uses Windows Console API functions to enable raw mode
//! (disabling line input and echo) and control the cursor position directly.

use crate::{KeyEvent, Terminal};
use std::io::{self, Write};
use winapi::um::consoleapi::{GetConsoleMode, ReadConsoleInputW, SetConsoleMode, SetConsoleCtrlHandler, WriteConsoleA};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::processenv::GetStdHandle;
use winapi::um::winbase::{STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
use winapi::um::wincon::{
    FillConsoleOutputAttribute, FillConsoleOutputCharacterA, GetConsoleScreenBufferInfo, SetConsoleCursorPosition,
    CONSOLE_SCREEN_BUFFER_INFO, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT,
    ENABLE_PROCESSED_INPUT, ENABLE_WINDOW_INPUT, INPUT_RECORD, KEY_EVENT, LEFT_CTRL_PRESSED,
    RIGHT_CTRL_PRESSED,
};
use winapi::um::wincontypes::KEY_EVENT_RECORD;
use winapi::um::winuser::{VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT, VK_UP};
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
/// let terminal = StdioTerminal::new();
/// ```
pub struct StdioTerminal {
    stdin_handle: HANDLE,
    stdout_handle: HANDLE,
    original_mode: Option<u32>,
    ctrl_handler_disabled: bool,
}

impl StdioTerminal {
    /// Creates a new Windows terminal using stdin/stdout handles.
    ///
    /// # Panics
    ///
    /// Panics if the standard handles cannot be obtained (extremely rare).
    pub fn new() -> Self {
        unsafe {
            let stdin_handle = GetStdHandle(STD_INPUT_HANDLE);
            let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);

            if stdin_handle == INVALID_HANDLE_VALUE || stdout_handle == INVALID_HANDLE_VALUE {
                panic!("Failed to get standard handles: {:?}", io::Error::last_os_error());
            }

            Self {
                stdin_handle,
                stdout_handle,
                original_mode: None,
                ctrl_handler_disabled: false,
            }
        }
    }
}

impl Default for StdioTerminal {
    fn default() -> Self {
        Self::new()
    }
}

impl Terminal for StdioTerminal {
    fn read_byte(&mut self) -> crate::Result<u8> {
        // This method is not used on Windows - we use ReadConsoleInputW instead
        // But we need to implement it for the trait
        Err(crate::Error::Io("read_byte not used on Windows"))
    }

    fn write(&mut self, data: &[u8]) -> crate::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        unsafe {
            let mut written: u32 = 0;
            if WriteConsoleA(
                self.stdout_handle,
                data.as_ptr() as *const _,
                data.len() as u32,
                &mut written,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }
        }

        Ok(())
    }

    fn flush(&mut self) -> crate::Result<()> {
        io::stdout().flush().map_err(|e| e.into())
    }

    fn enter_raw_mode(&mut self) -> crate::Result<()> {
        unsafe {
            let mut mode: u32 = 0;
            if GetConsoleMode(self.stdin_handle, &mut mode) == 0 {
                return Err(io::Error::last_os_error().into());
            }

            self.original_mode = Some(mode);

            // Disable line input, echo, and window input events
            let new_mode = mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT | ENABLE_WINDOW_INPUT);

            if SetConsoleMode(self.stdin_handle, new_mode) == 0 {
                return Err(io::Error::last_os_error().into());
            }

            // Disable Ctrl-C signal handler so we can handle it ourselves
            if SetConsoleCtrlHandler(None, 1) != 0 {
                self.ctrl_handler_disabled = true;
            }
        }

        Ok(())
    }

    fn exit_raw_mode(&mut self) -> crate::Result<()> {
        unsafe {
            // Re-enable Ctrl-C signal handler
            if self.ctrl_handler_disabled {
                SetConsoleCtrlHandler(None, 0);
                self.ctrl_handler_disabled = false;
            }

            if let Some(original) = self.original_mode {
                if SetConsoleMode(self.stdin_handle, original) == 0 {
                    return Err(io::Error::last_os_error().into());
                }
                self.original_mode = None;
            }
        }

        Ok(())
    }

    fn cursor_left(&mut self) -> crate::Result<()> {
        unsafe {
            let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(self.stdout_handle, &mut csbi) == 0 {
                return Err(io::Error::last_os_error().into());
            }

            let mut coord = csbi.dwCursorPosition;
            if coord.X > 0 {
                coord.X -= 1;
            }

            if SetConsoleCursorPosition(self.stdout_handle, coord) == 0 {
                return Err(io::Error::last_os_error().into());
            }
        }

        Ok(())
    }

    fn cursor_right(&mut self) -> crate::Result<()> {
        unsafe {
            let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(self.stdout_handle, &mut csbi) == 0 {
                return Err(io::Error::last_os_error().into());
            }

            let mut coord = csbi.dwCursorPosition;
            coord.X += 1;

            if SetConsoleCursorPosition(self.stdout_handle, coord) == 0 {
                return Err(io::Error::last_os_error().into());
            }
        }

        Ok(())
    }

    fn clear_eol(&mut self) -> crate::Result<()> {
        unsafe {
            let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(self.stdout_handle, &mut csbi) == 0 {
                return Err(io::Error::last_os_error().into());
            }

            let coord = csbi.dwCursorPosition;
            let count = (csbi.dwSize.X - coord.X) as u32;
            let mut written: u32 = 0;

            // Fill with spaces
            if FillConsoleOutputCharacterA(
                self.stdout_handle,
                b' ' as i8,
                count,
                coord,
                &mut written,
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }

            // Fill with default attributes
            if FillConsoleOutputAttribute(
                self.stdout_handle,
                csbi.wAttributes,
                count,
                coord,
                &mut written,
            ) == 0
            {
                return Err(io::Error::last_os_error().into());
            }

            // Restore cursor position
            if SetConsoleCursorPosition(self.stdout_handle, coord) == 0 {
                return Err(io::Error::last_os_error().into());
            }
        }

        Ok(())
    }

    fn parse_key_event(&mut self) -> crate::Result<KeyEvent> {
        loop {
            unsafe {
                let mut input_record: INPUT_RECORD = std::mem::zeroed();
                let mut events_read: u32 = 0;

                if ReadConsoleInputW(
                    self.stdin_handle,
                    &mut input_record,
                    1,
                    &mut events_read,
                ) == 0
                {
                    return Err(io::Error::last_os_error().into());
                }

                if events_read == 0 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF").into());
                }

                // Only process keyboard events
                if input_record.EventType != KEY_EVENT {
                    continue;
                }

                let key_event: KEY_EVENT_RECORD = *input_record.Event.KeyEvent();

                // Only process key down events
                if key_event.bKeyDown == 0 {
                    continue;
                }

                let vk_code = key_event.wVirtualKeyCode;
                let ctrl_pressed = (key_event.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED)) != 0;
                let char_code = *key_event.uChar.UnicodeChar();

                // Check for Ctrl+C first (VK code 'C' = 0x43)
                if ctrl_pressed && vk_code == 0x43 {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "Interrupted (Ctrl-C)"
                    ).into());
                }

                // Check for Ctrl+D (VK code 'D' = 0x44)
                if ctrl_pressed && vk_code == 0x44 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "EOF (Ctrl-D)"
                    ).into());
                }

                // Handle special keys
                match vk_code as i32 {
                    VK_RETURN => return Ok(KeyEvent::Enter),
                    VK_BACK => return Ok(KeyEvent::Backspace),
                    VK_DELETE => {
                        if ctrl_pressed {
                            return Ok(KeyEvent::CtrlDelete);
                        } else {
                            return Ok(KeyEvent::Delete);
                        }
                    }
                    VK_LEFT => {
                        if ctrl_pressed {
                            return Ok(KeyEvent::CtrlLeft);
                        } else {
                            return Ok(KeyEvent::Left);
                        }
                    }
                    VK_RIGHT => {
                        if ctrl_pressed {
                            return Ok(KeyEvent::CtrlRight);
                        } else {
                            return Ok(KeyEvent::Right);
                        }
                    }
                    VK_UP => return Ok(KeyEvent::Up),
                    VK_DOWN => return Ok(KeyEvent::Down),
                    VK_HOME => return Ok(KeyEvent::Home),
                    VK_END => return Ok(KeyEvent::End),
                    _ => {}
                }

                // Normal printable character
                if char_code >= 32 && char_code < 127 {
                    return Ok(KeyEvent::Normal(char_code as u8 as char));
                }

                // Ignore other characters
            }
        }
    }
}

impl Drop for StdioTerminal {
    fn drop(&mut self) {
        let _ = self.exit_raw_mode();
    }
}
