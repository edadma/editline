//! Synchronous line editor implementation.
//!
//! This module provides the blocking/synchronous version of the line editor,
//! suitable for standard terminals and embedded systems without async runtimes.

use crate::{Result, KeyEvent, LineBuffer, History};
use alloc::string::{String, ToString};

/// Terminal abstraction that enables platform-agnostic line editing.
///
/// Implement this trait to use editline with any I/O system: standard terminals,
/// UART connections, network sockets, or custom devices.
///
/// # Platform Implementations
///
/// This library provides built-in implementations:
/// - [`terminals::StdioTerminal`](crate::terminals::StdioTerminal) for Unix (termios + ANSI)
/// - [`terminals::StdioTerminal`](crate::terminals::StdioTerminal) for Windows (Console API)
///
/// # Example
///
/// ```
/// use editline::{Terminal, KeyEvent, Result};
///
/// struct MockTerminal {
///     input: Vec<u8>,
///     output: Vec<u8>,
/// }
///
/// impl Terminal for MockTerminal {
///     fn read_byte(&mut self) -> Result<u8> {
///         self.input.pop().ok_or(editline::Error::Eof)
///     }
///
///     fn write(&mut self, data: &[u8]) -> Result<()> {
///         self.output.extend_from_slice(data);
///         Ok(())
///     }
///
///     // ... implement other methods
/// #   fn flush(&mut self) -> Result<()> { Ok(()) }
/// #   fn enter_raw_mode(&mut self) -> Result<()> { Ok(()) }
/// #   fn exit_raw_mode(&mut self) -> Result<()> { Ok(()) }
/// #   fn cursor_left(&mut self) -> Result<()> { Ok(()) }
/// #   fn cursor_right(&mut self) -> Result<()> { Ok(()) }
/// #   fn clear_eol(&mut self) -> Result<()> { Ok(()) }
/// #   fn parse_key_event(&mut self) -> Result<KeyEvent> { Ok(KeyEvent::Enter) }
/// }
/// ```
pub trait Terminal {
    /// Reads a single byte from the input source.
    ///
    /// This is called repeatedly to fetch user input. Should block until a byte is available.
    fn read_byte(&mut self) -> Result<u8>;

    /// Writes raw bytes to the output.
    ///
    /// Used to display typed characters and redraw the line during editing.
    fn write(&mut self, data: &[u8]) -> Result<()>;

    /// Flushes any buffered output.
    ///
    /// Called after each key event to ensure immediate visual feedback.
    fn flush(&mut self) -> Result<()>;

    /// Enters raw mode for character-by-character input.
    ///
    /// Should disable line buffering and echo. Called at the start of [`LineEditor::read_line`].
    fn enter_raw_mode(&mut self) -> Result<()>;

    /// Exits raw mode and restores normal terminal settings.
    ///
    /// Called at the end of [`LineEditor::read_line`] to restore the terminal state.
    fn exit_raw_mode(&mut self) -> Result<()>;

    /// Moves the cursor left by one position.
    ///
    /// Typically outputs an ANSI escape sequence like `\x1b[D` or calls a platform API.
    fn cursor_left(&mut self) -> Result<()>;

    /// Moves the cursor right by one position.
    ///
    /// Typically outputs an ANSI escape sequence like `\x1b[C` or calls a platform API.
    fn cursor_right(&mut self) -> Result<()>;

    /// Clears from the cursor position to the end of the line.
    ///
    /// Typically outputs an ANSI escape sequence like `\x1b[K` or calls a platform API.
    fn clear_eol(&mut self) -> Result<()>;

    /// Parses the next key event from input.
    ///
    /// Should handle multi-byte sequences (like ANSI escape codes) and return a single
    /// [`KeyEvent`]. Called once per key press by [`LineEditor::read_line`].
    fn parse_key_event(&mut self) -> Result<KeyEvent>;
}

/// Main line editor interface with full editing and history support.
///
/// Provides a high-level API for reading edited lines from any [`Terminal`]
/// implementation. Handles all keyboard input, cursor movement, text editing,
/// and history navigation.
///
/// # Examples
///
/// ```no_run
/// use editline::{LineEditor, terminals::StdioTerminal};
///
/// let mut editor = LineEditor::new(1024, 50);
/// let mut terminal = StdioTerminal::new();
///
/// match editor.read_line(&mut terminal) {
///     Ok(line) => println!("Got: {}", line),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
///
/// # Key Bindings
///
/// - **Arrow keys**: Move cursor left/right, navigate history up/down
/// - **Home/End**: Jump to start/end of line
/// - **Backspace/Delete**: Delete characters
/// - **Ctrl+Left/Right**: Move by word
/// - **Alt+Backspace**: Delete word left
/// - **Ctrl+Delete**: Delete word right
/// - **Enter**: Submit line
pub struct LineEditor {
    line: LineBuffer,
    history: History,
}

impl LineEditor {
    /// Creates a new line editor with the specified capacities.
    ///
    /// # Arguments
    ///
    /// * `buffer_capacity` - Initial capacity for the line buffer in bytes
    /// * `history_capacity` - Maximum number of history entries to store
    ///
    /// # Examples
    ///
    /// ```
    /// use editline::LineEditor;
    ///
    /// // 1024 byte buffer, 50 history entries
    /// let editor = LineEditor::new(1024, 50);
    /// ```
    pub fn new(buffer_capacity: usize, history_capacity: usize) -> Self {
        Self {
            line: LineBuffer::new(buffer_capacity),
            history: History::new(history_capacity),
        }
    }

    /// Reads a line from the terminal with full editing support.
    ///
    /// Enters raw mode, processes key events until Enter is pressed, then returns
    /// the edited line with leading and trailing whitespace removed. The trimmed
    /// line is automatically added to history if non-empty.
    ///
    /// # Arguments
    ///
    /// * `terminal` - Any type implementing the [`Terminal`] trait
    ///
    /// # Returns
    ///
    /// `Ok(String)` with the trimmed entered line, or `Err` if an I/O error occurs.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use editline::{LineEditor, terminals::StdioTerminal};
    ///
    /// let mut editor = LineEditor::new(1024, 50);
    /// let mut terminal = StdioTerminal::new();
    ///
    /// print!("> ");
    /// std::io::Write::flush(&mut std::io::stdout()).unwrap();
    ///
    /// let line = editor.read_line(&mut terminal)?;
    /// println!("You entered: {}", line);
    /// # Ok::<(), editline::Error>(())
    /// ```
    pub fn read_line<T: Terminal>(&mut self, terminal: &mut T) -> Result<String> {
        self.line.clear();
        terminal.enter_raw_mode()?;

        // Use a closure to ensure we always exit raw mode, even on error
        let result = (|| {
            loop {
                let event = terminal.parse_key_event()?;

                if event == KeyEvent::Enter {
                    break;
                }

                self.handle_key_event(terminal, event)?;
            }

            // Platform-specific line ending
            // Unix/Linux/macOS uses \n, but embedded serial terminals need \r\n
            #[cfg(not(feature = "std"))]
            terminal.write(b"\r\n")?;
            #[cfg(feature = "std")]
            terminal.write(b"\n")?;
            terminal.flush()?;

            let result = self.line.as_str()?
                .trim()
                .to_string();

            // Add to history (History::add will check if empty and skip duplicates)
            self.history.add(&result);
            self.history.reset_view();

            Ok(result)
        })();

        // Always exit raw mode, even if an error occurred
        terminal.exit_raw_mode()?;

        result
    }

    fn handle_key_event<T: Terminal>(&mut self, terminal: &mut T, event: KeyEvent) -> Result<()> {
        match event {
            KeyEvent::Normal(c) => {
                self.history.reset_view();
                self.line.insert_char(c);
                terminal.write(c.to_string().as_bytes())?;
                self.redraw_from_cursor(terminal)?;
            }
            KeyEvent::Left => {
                if self.line.move_cursor_left() {
                    terminal.cursor_left()?;
                }
            }
            KeyEvent::Right => {
                if self.line.move_cursor_right() {
                    terminal.cursor_right()?;
                }
            }
            KeyEvent::Up => {
                let current = self.line.as_str().unwrap_or("").to_string();
                if let Some(text) = self.history.previous(&current) {
                    let text = text.to_string();
                    self.load_history_into_line(terminal, &text)?;
                }
            }
            KeyEvent::Down => {
                if let Some(text) = self.history.next_entry() {
                    let text = text.to_string();
                    self.load_history_into_line(terminal, &text)?;
                }
                // If None, we're not viewing history, so do nothing
            }
            KeyEvent::Home => {
                let count = self.line.move_cursor_to_start();
                for _ in 0..count {
                    terminal.cursor_left()?;
                }
            }
            KeyEvent::End => {
                let count = self.line.move_cursor_to_end();
                for _ in 0..count {
                    terminal.cursor_right()?;
                }
            }
            KeyEvent::Backspace => {
                self.history.reset_view();
                if self.line.delete_before_cursor() {
                    terminal.cursor_left()?;
                    self.redraw_from_cursor(terminal)?;
                }
            }
            KeyEvent::Delete => {
                self.history.reset_view();
                if self.line.delete_at_cursor() {
                    self.redraw_from_cursor(terminal)?;
                }
            }
            KeyEvent::CtrlLeft => {
                let count = self.line.move_cursor_word_left();
                for _ in 0..count {
                    terminal.cursor_left()?;
                }
            }
            KeyEvent::CtrlRight => {
                let count = self.line.move_cursor_word_right();
                for _ in 0..count {
                    terminal.cursor_right()?;
                }
            }
            KeyEvent::AltBackspace => {
                self.history.reset_view();
                let count = self.line.delete_word_left();
                for _ in 0..count {
                    terminal.cursor_left()?;
                }
                self.redraw_from_cursor(terminal)?;
            }
            KeyEvent::CtrlDelete => {
                self.history.reset_view();
                self.line.delete_word_right();
                self.redraw_from_cursor(terminal)?;
            }
            KeyEvent::Enter => {}
        }

        terminal.flush()?;
        Ok(())
    }

    fn redraw_from_cursor<T: Terminal>(&self, terminal: &mut T) -> Result<()> {
        terminal.clear_eol()?;

        let cursor_pos = self.line.cursor_pos();
        let remaining = &self.line.as_bytes()[cursor_pos..];
        terminal.write(remaining)?;

        // Move cursor back
        for _ in 0..remaining.len() {
            terminal.cursor_left()?;
        }

        Ok(())
    }

    fn clear_line_display<T: Terminal>(&self, terminal: &mut T) -> Result<()> {
        for _ in 0..self.line.cursor_pos() {
            terminal.cursor_left()?;
        }
        terminal.clear_eol()?;
        Ok(())
    }

    fn load_history_into_line<T: Terminal>(&mut self, terminal: &mut T, text: &str) -> Result<()> {
        self.clear_line_display(terminal)?;
        self.line.load(text);
        terminal.write(text.as_bytes())?;
        Ok(())
    }
}
