//! Asynchronous line editor implementation.
//!
//! This module provides the async version of the line editor,
//! suitable for async runtimes like Embassy on embedded systems.

use crate::{Result, KeyEvent, LineBuffer, History};
use alloc::string::{String, ToString};

/// Asynchronous terminal abstraction for async runtimes.
///
/// This is the async counterpart to [`Terminal`](crate::Terminal), designed for
/// use with async runtimes like Embassy on embedded systems. All I/O operations
/// are async and return futures.
///
/// # Platform Implementations
///
/// This library provides built-in implementations:
/// - [`terminals::EmbassyUsbTerminal`](crate::terminals::EmbassyUsbTerminal) for Embassy USB CDC
///
/// # Example
///
/// ```ignore
/// use editline::{AsyncTerminal, KeyEvent, Result};
///
/// struct MyAsyncTerminal {
///     // Your platform-specific fields
/// }
///
/// impl AsyncTerminal for MyAsyncTerminal {
///     async fn read_byte(&mut self) -> Result<u8> {
///         // Read from your input source
///         Ok(b'x')
///     }
///
///     async fn write(&mut self, data: &[u8]) -> Result<()> {
///         // Write to your output
///         Ok(())
///     }
///
///     async fn flush(&mut self) -> Result<()> {
///         // Flush output
///         Ok(())
///     }
///
///     async fn enter_raw_mode(&mut self) -> Result<()> {
///         // Configure for character-by-character input
///         Ok(())
///     }
///
///     async fn exit_raw_mode(&mut self) -> Result<()> {
///         // Restore normal mode
///         Ok(())
///     }
///
///     async fn cursor_left(&mut self) -> Result<()> {
///         // Move cursor left one position
///         Ok(())
///     }
///
///     async fn cursor_right(&mut self) -> Result<()> {
///         // Move cursor right one position
///         Ok(())
///     }
///
///     async fn clear_eol(&mut self) -> Result<()> {
///         // Clear from cursor to end of line
///         Ok(())
///     }
///
///     async fn parse_key_event(&mut self) -> Result<KeyEvent> {
///         // Parse input bytes into key events
///         Ok(KeyEvent::Enter)
///     }
/// }
/// ```
pub trait AsyncTerminal {
    /// Reads a single byte from the input source.
    ///
    /// This is called repeatedly to fetch user input. Should await until a byte is available.
    async fn read_byte(&mut self) -> Result<u8>;

    /// Writes raw bytes to the output.
    ///
    /// Used to display typed characters and redraw the line during editing.
    async fn write(&mut self, data: &[u8]) -> Result<()>;

    /// Flushes any buffered output.
    ///
    /// Called after each key event to ensure immediate visual feedback.
    async fn flush(&mut self) -> Result<()>;

    /// Enters raw mode for character-by-character input.
    ///
    /// Should disable line buffering and echo. Called at the start of [`AsyncLineEditor::read_line`].
    async fn enter_raw_mode(&mut self) -> Result<()>;

    /// Exits raw mode and restores normal terminal settings.
    ///
    /// Called at the end of [`AsyncLineEditor::read_line`] to restore the terminal state.
    async fn exit_raw_mode(&mut self) -> Result<()>;

    /// Moves the cursor left by one position.
    ///
    /// Typically outputs an ANSI escape sequence like `\x1b[D` or calls a platform API.
    async fn cursor_left(&mut self) -> Result<()>;

    /// Moves the cursor right by one position.
    ///
    /// Typically outputs an ANSI escape sequence like `\x1b[C` or calls a platform API.
    async fn cursor_right(&mut self) -> Result<()>;

    /// Clears from the cursor position to the end of the line.
    ///
    /// Typically outputs an ANSI escape sequence like `\x1b[K` or calls a platform API.
    async fn clear_eol(&mut self) -> Result<()>;

    /// Parses the next key event from input.
    ///
    /// Should handle multi-byte sequences (like ANSI escape codes) and return a single
    /// [`KeyEvent`]. Called once per key press by [`AsyncLineEditor::read_line`].
    async fn parse_key_event(&mut self) -> Result<KeyEvent>;
}

/// Asynchronous line editor interface with full editing and history support.
///
/// Provides a high-level async API for reading edited lines from any [`AsyncTerminal`]
/// implementation. Handles all keyboard input, cursor movement, text editing,
/// and history navigation.
///
/// # Examples
///
/// ```ignore
/// use editline::{AsyncLineEditor, terminals::EmbassyUsbTerminal};
///
/// let mut editor = AsyncLineEditor::new(1024, 50);
/// let mut terminal = EmbassyUsbTerminal::new(usb_class);
///
/// match editor.read_line(&mut terminal).await {
///     Ok(line) => defmt::info!("Got: {}", line),
///     Err(e) => defmt::error!("Error: {:?}", e),
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
pub struct AsyncLineEditor {
    line: LineBuffer,
    history: History,
}

impl AsyncLineEditor {
    /// Creates a new async line editor with the specified capacities.
    ///
    /// # Arguments
    ///
    /// * `buffer_capacity` - Initial capacity for the line buffer in bytes
    /// * `history_capacity` - Maximum number of history entries to store
    ///
    /// # Examples
    ///
    /// ```
    /// use editline::AsyncLineEditor;
    ///
    /// // 1024 byte buffer, 50 history entries
    /// let editor = AsyncLineEditor::new(1024, 50);
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
    /// * `terminal` - Any type implementing the [`AsyncTerminal`] trait
    ///
    /// # Returns
    ///
    /// `Ok(String)` with the trimmed entered line, or `Err` if an I/O error occurs.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use editline::{AsyncLineEditor, terminals::EmbassyUsbTerminal};
    ///
    /// let mut editor = AsyncLineEditor::new(1024, 50);
    /// let mut terminal = EmbassyUsbTerminal::new(usb_class);
    ///
    /// let _ = terminal.write(b"> ").await;
    /// let _ = terminal.flush().await;
    ///
    /// let line = editor.read_line(&mut terminal).await?;
    /// defmt::info!("You entered: {}", line);
    /// # Ok::<(), editline::Error>(())
    /// ```
    pub async fn read_line<T: AsyncTerminal>(&mut self, terminal: &mut T) -> Result<String> {
        self.line.clear();
        terminal.enter_raw_mode().await?;

        // Use a closure to ensure we always exit raw mode, even on error
        let result = async {
            loop {
                let event = terminal.parse_key_event().await?;

                if event == KeyEvent::Enter {
                    break;
                }

                self.handle_key_event(terminal, event).await?;
            }

            // Embedded serial terminals need \r\n
            terminal.write(b"\r\n").await?;
            terminal.flush().await?;

            let result = self.line.as_str()?
                .trim()
                .to_string();

            // Add to history (History::add will check if empty and skip duplicates)
            self.history.add(&result);
            self.history.reset_view();

            Ok(result)
        }.await;

        // Always exit raw mode, even if an error occurred
        let _ = terminal.exit_raw_mode().await;

        result
    }

    async fn handle_key_event<T: AsyncTerminal>(&mut self, terminal: &mut T, event: KeyEvent) -> Result<()> {
        match event {
            KeyEvent::Normal(c) => {
                self.history.reset_view();
                self.line.insert_char(c);
                terminal.write(c.to_string().as_bytes()).await?;
                self.redraw_from_cursor(terminal).await?;
            }
            KeyEvent::Left => {
                if self.line.move_cursor_left() {
                    terminal.cursor_left().await?;
                }
            }
            KeyEvent::Right => {
                if self.line.move_cursor_right() {
                    terminal.cursor_right().await?;
                }
            }
            KeyEvent::Up => {
                let current = self.line.as_str().unwrap_or("").to_string();
                if let Some(text) = self.history.previous(&current) {
                    let text = text.to_string();
                    self.load_history_into_line(terminal, &text).await?;
                }
            }
            KeyEvent::Down => {
                if let Some(text) = self.history.next_entry() {
                    let text = text.to_string();
                    self.load_history_into_line(terminal, &text).await?;
                }
                // If None, we're not viewing history, so do nothing
            }
            KeyEvent::Home => {
                let count = self.line.move_cursor_to_start();
                for _ in 0..count {
                    terminal.cursor_left().await?;
                }
            }
            KeyEvent::End => {
                let count = self.line.move_cursor_to_end();
                for _ in 0..count {
                    terminal.cursor_right().await?;
                }
            }
            KeyEvent::Backspace => {
                self.history.reset_view();
                if self.line.delete_before_cursor() {
                    terminal.cursor_left().await?;
                    self.redraw_from_cursor(terminal).await?;
                }
            }
            KeyEvent::Delete => {
                self.history.reset_view();
                if self.line.delete_at_cursor() {
                    self.redraw_from_cursor(terminal).await?;
                }
            }
            KeyEvent::CtrlLeft => {
                let count = self.line.move_cursor_word_left();
                for _ in 0..count {
                    terminal.cursor_left().await?;
                }
            }
            KeyEvent::CtrlRight => {
                let count = self.line.move_cursor_word_right();
                for _ in 0..count {
                    terminal.cursor_right().await?;
                }
            }
            KeyEvent::AltBackspace => {
                self.history.reset_view();
                let count = self.line.delete_word_left();
                for _ in 0..count {
                    terminal.cursor_left().await?;
                }
                self.redraw_from_cursor(terminal).await?;
            }
            KeyEvent::CtrlDelete => {
                self.history.reset_view();
                self.line.delete_word_right();
                self.redraw_from_cursor(terminal).await?;
            }
            KeyEvent::Enter => {}
        }

        terminal.flush().await?;
        Ok(())
    }

    async fn redraw_from_cursor<T: AsyncTerminal>(&self, terminal: &mut T) -> Result<()> {
        terminal.clear_eol().await?;

        let cursor_pos = self.line.cursor_pos();
        let remaining = &self.line.as_bytes()[cursor_pos..];
        terminal.write(remaining).await?;

        // Move cursor back
        for _ in 0..remaining.len() {
            terminal.cursor_left().await?;
        }

        Ok(())
    }

    async fn clear_line_display<T: AsyncTerminal>(&self, terminal: &mut T) -> Result<()> {
        for _ in 0..self.line.cursor_pos() {
            terminal.cursor_left().await?;
        }
        terminal.clear_eol().await?;
        Ok(())
    }

    async fn load_history_into_line<T: AsyncTerminal>(&mut self, terminal: &mut T, text: &str) -> Result<()> {
        self.clear_line_display(terminal).await?;
        self.line.load(text);
        terminal.write(text.as_bytes()).await?;
        Ok(())
    }
}
