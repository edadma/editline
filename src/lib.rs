//! Platform-agnostic line editor with history and full editing capabilities.
//!
//! This library provides a flexible line editing system with complete separation between
//! I/O operations and editing logic. This design enables usage across various platforms
//! and I/O systems without modification to the core logic.
//!
//! # Features
//!
//! - **Full line editing**: Insert, delete, cursor movement
//! - **Word-aware navigation**: Ctrl+Left/Right, Alt+Backspace, Ctrl+Delete
//! - **Command history**: 50-entry circular buffer with up/down navigation
//! - **Smart history**: Automatically skips duplicates and empty lines
//! - **Cross-platform**: Unix (termios/ANSI) and Windows (Console API) implementations included
//! - **Sync and Async**: Both blocking and async APIs available via feature flags
//! - **Zero global state**: All state is explicitly managed
//! - **Type-safe**: Strong typing with Result-based error handling
//!
//! # Quick Start (Sync)
//!
//! ```no_run
//! use editline::{LineEditor, terminals::StdioTerminal};
//!
//! let mut editor = LineEditor::new(1024, 50);  // buffer size, history size
//! let mut terminal = StdioTerminal::new();
//!
//! loop {
//!     print!("> ");
//!     std::io::Write::flush(&mut std::io::stdout()).unwrap();
//!
//!     match editor.read_line(&mut terminal) {
//!         Ok(line) => {
//!             if line == "exit" {
//!                 break;
//!             }
//!             println!("You typed: {}", line);
//!         }
//!         Err(e) => {
//!             eprintln!("Error: {}", e);
//!             break;
//!         }
//!     }
//! }
//! ```
//!
//! # Quick Start (Async)
//!
//! ```ignore
//! use editline::{AsyncLineEditor, terminals::EmbassyUsbTerminal};
//!
//! let mut editor = AsyncLineEditor::new(1024, 50);
//! let mut terminal = EmbassyUsbTerminal::new(usb_class);
//!
//! loop {
//!     let _ = terminal.write(b"> ").await;
//!     let _ = terminal.flush().await;
//!
//!     match editor.read_line(&mut terminal).await {
//!         Ok(line) => {
//!             if line == "exit" {
//!                 break;
//!             }
//!             defmt::info!("You typed: {}", line);
//!         }
//!         Err(e) => {
//!             defmt::error!("Error: {:?}", e);
//!             break;
//!         }
//!     }
//! }
//! ```
//!
//! # Architecture
//!
//! The library is organized around several components:
//!
//! - **Shared Components** (work with both sync and async):
//!   - [`LineBuffer`]: Manages text buffer and cursor position
//!   - [`History`]: Circular buffer for command history
//!   - [`KeyEvent`]: Key event enumeration
//!   - [`Error`]: Error type
//!
//! - **Sync API** (feature = "sync", default):
//!   - [`Terminal`]: Blocking I/O trait
//!   - [`LineEditor`]: Blocking line editor
//!
//! - **Async API** (feature = "async"):
//!   - [`AsyncTerminal`]: Async I/O trait
//!   - [`AsyncLineEditor`]: Async line editor

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use core::option::Option::{self, Some, None};
use core::convert::From;

/// Error type for editline operations
#[derive(Debug)]
pub enum Error {
    /// I/O error occurred
    Io(&'static str),
    /// Invalid UTF-8 data
    InvalidUtf8,
    /// End of file
    Eof,
    /// Operation interrupted
    Interrupted,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(msg) => {
                f.write_str("I/O error: ")?;
                f.write_str(msg)
            }
            Error::InvalidUtf8 => f.write_str("Invalid UTF-8"),
            Error::Eof => f.write_str("End of file"),
            Error::Interrupted => f.write_str("Interrupted"),
        }
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match e.kind() {
            ErrorKind::UnexpectedEof => Error::Eof,
            ErrorKind::Interrupted => Error::Interrupted,
            _ => Error::Io("I/O error"),
        }
    }
}

#[cfg(feature = "std")]
impl From<Error> for std::io::Error {
    fn from(e: Error) -> Self {
        use std::io::{Error as IoError, ErrorKind};
        match e {
            Error::Io(msg) => IoError::new(ErrorKind::Other, msg),
            Error::InvalidUtf8 => IoError::new(ErrorKind::InvalidData, "Invalid UTF-8"),
            Error::Eof => IoError::new(ErrorKind::UnexpectedEof, "End of file"),
            Error::Interrupted => IoError::new(ErrorKind::Interrupted, "Interrupted"),
        }
    }
}

impl From<core::str::Utf8Error> for Error {
    fn from(_: core::str::Utf8Error) -> Self {
        Error::InvalidUtf8
    }
}

/// Result type for editline operations
pub type Result<T> = core::result::Result<T, Error>;

/// Key events that can be processed by the line editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    /// Normal printable character
    Normal(char),
    /// Left arrow
    Left,
    /// Right arrow
    Right,
    /// Up arrow (history previous)
    Up,
    /// Down arrow (history next)
    Down,
    /// Home key
    Home,
    /// End key
    End,
    /// Backspace
    Backspace,
    /// Delete
    Delete,
    /// Enter/Return
    Enter,
    /// Ctrl+Left (word left)
    CtrlLeft,
    /// Ctrl+Right (word right)
    CtrlRight,
    /// Ctrl+Delete (delete word right)
    CtrlDelete,
    /// Alt+Backspace (delete word left)
    AltBackspace,
}

/// Text buffer with cursor tracking for line editing operations.
///
/// Manages the actual text being edited and the cursor position within it.
/// Supports UTF-8 text and provides methods for character/word manipulation.
///
/// This struct is typically not used directly - instead use [`LineEditor`] which
/// provides the high-level editing interface.
pub struct LineBuffer {
    buffer: Vec<u8>,
    cursor_pos: usize,
}

impl LineBuffer {
    /// Creates a new line buffer with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Initial capacity for the internal buffer in bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use editline::LineBuffer;
    ///
    /// let buffer = LineBuffer::new(1024);
    /// assert!(buffer.is_empty());
    /// ```
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            cursor_pos: 0,
        }
    }

    /// Clears the buffer and resets the cursor to the start.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor_pos = 0;
    }

    /// Returns the length of the buffer in bytes.
    ///
    /// Note: For UTF-8 text, this is the byte count, not the character count.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Returns the current cursor position in bytes from the start.
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Returns the buffer contents as a UTF-8 string slice.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the buffer contains invalid UTF-8.
    pub fn as_str(&self) -> Result<&str> {
        core::str::from_utf8(&self.buffer).map_err(|_| Error::InvalidUtf8)
    }

    /// Returns the buffer contents as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer
    }

    /// Inserts a character at the cursor position, moving the cursor forward.
    ///
    /// Supports UTF-8 characters. The cursor advances by the byte length of the character.
    pub fn insert_char(&mut self, c: char) {
        let mut buf = [0; 4];
        let bytes = c.encode_utf8(&mut buf).as_bytes();

        for &byte in bytes {
            self.buffer.insert(self.cursor_pos, byte);
            self.cursor_pos += 1;
        }
    }

    /// Deletes the character before the cursor (backspace operation).
    ///
    /// Returns `true` if a character was deleted, `false` if the cursor is at the start.
    pub fn delete_before_cursor(&mut self) -> bool {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.buffer.remove(self.cursor_pos);
            true
        } else {
            false
        }
    }

    /// Deletes the character at the cursor (delete key operation).
    ///
    /// Returns `true` if a character was deleted, `false` if the cursor is at the end.
    pub fn delete_at_cursor(&mut self) -> bool {
        if self.cursor_pos < self.buffer.len() {
            self.buffer.remove(self.cursor_pos);
            true
        } else {
            false
        }
    }

    /// Moves the cursor one position to the left.
    ///
    /// Returns `true` if the cursor moved, `false` if already at the start.
    pub fn move_cursor_left(&mut self) -> bool {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            true
        } else {
            false
        }
    }

    /// Moves the cursor one position to the right.
    ///
    /// Returns `true` if the cursor moved, `false` if already at the end.
    pub fn move_cursor_right(&mut self) -> bool {
        if self.cursor_pos < self.buffer.len() {
            self.cursor_pos += 1;
            true
        } else {
            false
        }
    }

    /// Moves the cursor to the start of the line.
    ///
    /// Returns the number of positions the cursor moved.
    pub fn move_cursor_to_start(&mut self) -> usize {
        let old_pos = self.cursor_pos;
        self.cursor_pos = 0;
        old_pos
    }

    /// Moves the cursor to the end of the line.
    ///
    /// Returns the number of positions the cursor moved.
    pub fn move_cursor_to_end(&mut self) -> usize {
        let old_pos = self.cursor_pos;
        self.cursor_pos = self.buffer.len();
        self.buffer.len() - old_pos
    }

    /// Find start of word to the left
    fn find_word_start_left(&self) -> usize {
        if self.cursor_pos == 0 {
            return 0;
        }

        let mut pos = self.cursor_pos;

        // Skip any trailing whitespace first
        while pos > 0 && is_whitespace(self.buffer[pos - 1]) {
            pos -= 1;
        }

        if pos == 0 {
            return 0;
        }

        // Now we're on a non-whitespace character
        // Skip characters of the same type (word chars or symbols)
        let is_word = is_word_char(self.buffer[pos - 1]);
        while pos > 0 {
            let c = self.buffer[pos - 1];
            if is_whitespace(c) {
                break;
            }
            if is_word != is_word_char(c) {
                break;
            }
            pos -= 1;
        }

        pos
    }

    /// Find start of word to the right
    fn find_word_start_right(&self) -> usize {
        if self.cursor_pos >= self.buffer.len() {
            return self.buffer.len();
        }

        let mut pos = self.cursor_pos;

        // Skip characters of the same type (word chars or symbols)
        let is_word = is_word_char(self.buffer[pos]);
        while pos < self.buffer.len() {
            let c = self.buffer[pos];
            if is_whitespace(c) {
                break;
            }
            if is_word != is_word_char(c) {
                break;
            }
            pos += 1;
        }

        // Skip whitespace
        while pos < self.buffer.len() && is_whitespace(self.buffer[pos]) {
            pos += 1;
        }

        pos
    }

    /// Moves the cursor to the start of the previous word.
    ///
    /// Words are defined as sequences of alphanumeric characters and underscores.
    /// Symbols (like `+`, `-`, `*`) are treated as separate words. Only whitespace
    /// is skipped when navigating between words.
    ///
    /// Returns the number of positions the cursor moved.
    pub fn move_cursor_word_left(&mut self) -> usize {
        let target = self.find_word_start_left();
        let moved = self.cursor_pos - target;
        self.cursor_pos = target;
        moved
    }

    /// Moves the cursor to the start of the next word.
    ///
    /// Words are defined as sequences of alphanumeric characters and underscores.
    /// Symbols (like `+`, `-`, `*`) are treated as separate words. Only whitespace
    /// is skipped when navigating between words.
    ///
    /// Returns the number of positions the cursor moved.
    pub fn move_cursor_word_right(&mut self) -> usize {
        let target = self.find_word_start_right();
        let moved = target - self.cursor_pos;
        self.cursor_pos = target;
        moved
    }

    /// Deletes the word to the left of the cursor (Alt+Backspace operation).
    ///
    /// Returns the number of bytes deleted.
    pub fn delete_word_left(&mut self) -> usize {
        let target = self.find_word_start_left();
        let count = self.cursor_pos - target;

        for _ in 0..count {
            if self.cursor_pos > 0 {
                self.cursor_pos -= 1;
                self.buffer.remove(self.cursor_pos);
            }
        }

        count
    }

    /// Deletes the word to the right of the cursor (Ctrl+Delete operation).
    ///
    /// Returns the number of bytes deleted.
    pub fn delete_word_right(&mut self) -> usize {
        let target = self.find_word_start_right();
        let count = target - self.cursor_pos;

        for _ in 0..count {
            if self.cursor_pos < self.buffer.len() {
                self.buffer.remove(self.cursor_pos);
            }
        }

        count
    }

    /// Loads text into the buffer, replacing existing content.
    ///
    /// The cursor is positioned at the end of the loaded text.
    /// Used internally for history navigation.
    pub fn load(&mut self, text: &str) {
        self.buffer.clear();
        self.buffer.extend_from_slice(text.as_bytes());
        self.cursor_pos = self.buffer.len();
    }
}

/// Check if a byte is a word character (alphanumeric or underscore).
fn is_word_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Check if a byte is whitespace (space or tab).
fn is_whitespace(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

/// Command history manager with circular buffer storage.
///
/// Maintains a fixed-size history of entered commands with automatic
/// duplicate and empty-line filtering. Supports bidirectional navigation
/// and preserves the current line when browsing history.
///
/// # Examples
///
/// ```
/// use editline::History;
///
/// let mut hist = History::new(50);
/// hist.add("first command");
/// hist.add("second command");
///
/// // Navigate through history
/// assert_eq!(hist.previous(""), Some("second command"));
/// assert_eq!(hist.previous(""), Some("first command"));
/// ```
pub struct History {
    entries: Vec<String>,
    capacity: usize,
    current_entry: usize,
    viewing_entry: Option<usize>,
    saved_line: Option<String>,
}

impl History {
    /// Creates a new history buffer with the specified capacity.
    ///
    /// When the capacity is reached, the oldest entries are overwritten.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of history entries to store
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
            current_entry: 0,
            viewing_entry: None,
            saved_line: None,
        }
    }

    /// Adds a line to the history.
    ///
    /// Empty lines (including whitespace-only) and consecutive duplicates are automatically skipped.
    /// When the buffer is full, the oldest entry is overwritten.
    ///
    /// # Arguments
    ///
    /// * `line` - The command line to add to history
    pub fn add(&mut self, line: &str) {
        let trimmed = line.trim();

        // Skip empty or whitespace-only lines
        if trimmed.is_empty() {
            return;
        }

        // Skip if same as most recent (after trimming)
        if let Some(last) = self.entries.last() {
            if last == trimmed {
                return;
            }
        }

        if self.entries.len() < self.capacity {
            self.entries.push(trimmed.to_string());
            self.current_entry = self.entries.len() - 1;
        } else {
            // Circular buffer - overwrite oldest
            self.current_entry = (self.current_entry + 1) % self.capacity;
            self.entries[self.current_entry] = trimmed.to_string();
        }

        self.viewing_entry = None;
        self.saved_line = None;
    }

    /// Navigates to the previous (older) history entry.
    ///
    /// On the first call, saves `current_line` so it can be restored when
    /// navigating forward past the most recent entry.
    ///
    /// # Arguments
    ///
    /// * `current_line` - The current line content to save (only used on first call)
    ///
    /// # Returns
    ///
    /// `Some(&str)` with the previous history entry, or `None` if at the oldest entry.
    pub fn previous(&mut self, current_line: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }

        match self.viewing_entry {
            None => {
                // First time - save current line and start at most recent
                self.saved_line = Some(current_line.to_string());
                self.viewing_entry = Some(self.current_entry);
                Some(&self.entries[self.current_entry])
            }
            Some(idx) => {
                // Go further back
                if self.entries.len() < self.capacity {
                    // Haven't filled buffer yet
                    if idx > 0 {
                        let prev = idx - 1;
                        self.viewing_entry = Some(prev);
                        Some(&self.entries[prev])
                    } else {
                        None
                    }
                } else {
                    // Buffer is full
                    let prev = (idx + self.capacity - 1) % self.capacity;
                    if prev == self.current_entry {
                        None
                    } else {
                        self.viewing_entry = Some(prev);
                        Some(&self.entries[prev])
                    }
                }
            }
        }
    }

    /// Navigates to the next (newer) history entry.
    ///
    /// When reaching the end of history, returns the saved current line
    /// that was passed to the first [`previous`](Self::previous) call.
    ///
    /// # Returns
    ///
    /// `Some(&str)` with the next history entry or saved line, or `None` if
    /// not currently viewing history.
    pub fn next_entry(&mut self) -> Option<&str> {
        match self.viewing_entry {
            None => None,
            Some(idx) => {
                if self.entries.len() < self.capacity {
                    // Haven't filled buffer yet
                    if idx < self.entries.len() - 1 {
                        let next = idx + 1;
                        self.viewing_entry = Some(next);
                        Some(&self.entries[next])
                    } else {
                        // Reached the end, return saved line
                        self.viewing_entry = None;
                        self.saved_line.as_deref()
                    }
                } else {
                    // Buffer is full
                    let next = (idx + 1) % self.capacity;
                    if next == (self.current_entry + 1) % self.capacity {
                        // Reached the end, return saved line
                        self.viewing_entry = None;
                        self.saved_line.as_deref()
                    } else {
                        self.viewing_entry = Some(next);
                        Some(&self.entries[next])
                    }
                }
            }
        }
    }

    /// Resets the history view to the current line.
    ///
    /// Called when the user starts typing to exit history browsing mode.
    pub fn reset_view(&mut self) {
        self.viewing_entry = None;
    }
}

// Sync editor module
#[cfg(feature = "sync")]
mod sync_editor;

#[cfg(feature = "sync")]
pub use sync_editor::{Terminal, LineEditor};

// Async editor module
#[cfg(feature = "async")]
mod async_editor;

#[cfg(feature = "async")]
pub use async_editor::{AsyncTerminal, AsyncLineEditor};

// Re-export terminal implementations
#[cfg(any(feature = "std", feature = "microbit", feature = "rp_pico_usb", feature = "rp_pico2_usb", feature = "embassy_usb"))]
pub mod terminals;

#[cfg(test)]
mod tests {
    use super::*;

    // LineBuffer tests
    #[test]
    fn test_line_buffer_insert() {
        let mut buf = LineBuffer::new(100);
        buf.insert_char('h');
        buf.insert_char('i');
        assert_eq!(buf.as_str().unwrap(), "hi");
        assert_eq!(buf.cursor_pos(), 2);
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn test_line_buffer_backspace() {
        let mut buf = LineBuffer::new(100);
        buf.insert_char('h');
        buf.insert_char('i');
        assert!(buf.delete_before_cursor());
        assert_eq!(buf.as_str().unwrap(), "h");
        assert_eq!(buf.cursor_pos(), 1);
    }

    #[test]
    fn test_line_buffer_delete() {
        let mut buf = LineBuffer::new(100);
        buf.insert_char('h');
        buf.insert_char('i');
        buf.move_cursor_left();
        assert!(buf.delete_at_cursor());
        assert_eq!(buf.as_str().unwrap(), "h");
        assert_eq!(buf.cursor_pos(), 1);
    }

    #[test]
    fn test_line_buffer_cursor_movement() {
        let mut buf = LineBuffer::new(100);
        buf.insert_char('h');
        buf.insert_char('e');
        buf.insert_char('y');
        assert_eq!(buf.cursor_pos(), 3);

        assert!(buf.move_cursor_left());
        assert_eq!(buf.cursor_pos(), 2);

        assert!(buf.move_cursor_right());
        assert_eq!(buf.cursor_pos(), 3);

        assert!(!buf.move_cursor_right()); // at end
    }

    #[test]
    fn test_line_buffer_home_end() {
        let mut buf = LineBuffer::new(100);
        buf.insert_char('h');
        buf.insert_char('e');
        buf.insert_char('y');

        buf.move_cursor_to_start();
        assert_eq!(buf.cursor_pos(), 0);

        buf.move_cursor_to_end();
        assert_eq!(buf.cursor_pos(), 3);
    }

    #[test]
    fn test_line_buffer_word_navigation() {
        let mut buf = LineBuffer::new(100);
        for c in "hello world test".chars() {
            buf.insert_char(c);
        }

        // At end: "hello world test|"
        buf.move_cursor_word_left();
        assert_eq!(buf.cursor_pos(), 12); // "hello world |test"

        buf.move_cursor_word_left();
        assert_eq!(buf.cursor_pos(), 6); // "hello |world test"

        buf.move_cursor_word_right();
        assert_eq!(buf.cursor_pos(), 12); // "hello world |test"
    }

    #[test]
    fn test_line_buffer_delete_word() {
        let mut buf = LineBuffer::new(100);
        for c in "hello world".chars() {
            buf.insert_char(c);
        }

        buf.delete_word_left();
        assert_eq!(buf.as_str().unwrap(), "hello ");

        buf.delete_word_left();
        assert_eq!(buf.as_str().unwrap(), "");
    }

    #[test]
    fn test_line_buffer_delete_word_right() {
        let mut buf = LineBuffer::new(100);
        for c in "hello world".chars() {
            buf.insert_char(c);
        }
        buf.move_cursor_to_start();

        buf.delete_word_right();
        assert_eq!(buf.as_str().unwrap(), "world");
    }

    #[test]
    fn test_line_buffer_insert_middle() {
        let mut buf = LineBuffer::new(100);
        buf.insert_char('h');
        buf.insert_char('e');
        buf.move_cursor_left();
        buf.insert_char('x');
        assert_eq!(buf.as_str().unwrap(), "hxe");
        assert_eq!(buf.cursor_pos(), 2);
    }

    #[test]
    fn test_word_navigation_with_symbols() {
        let mut buf = LineBuffer::new(100);
        for c in "3 + 5".chars() {
            buf.insert_char(c);
        }
        // Cursor at end: "3 + 5|"

        // Move left by word - should stop at '5'
        buf.move_cursor_word_left();
        assert_eq!(buf.cursor_pos(), 4); // Before '5'

        // Move left by word - should stop at '+'
        buf.move_cursor_word_left();
        assert_eq!(buf.cursor_pos(), 2); // Before '+'

        // Move left by word - should stop at '3'
        buf.move_cursor_word_left();
        assert_eq!(buf.cursor_pos(), 0); // Before '3'

        // Move right by word - should stop after '3'
        buf.move_cursor_word_right();
        assert_eq!(buf.cursor_pos(), 2); // After '3 ', before '+'

        // Move right by word - should stop after '+'
        buf.move_cursor_word_right();
        assert_eq!(buf.cursor_pos(), 4); // After '+ ', before '5'
    }

    #[test]
    fn test_delete_word_with_symbols() {
        let mut buf = LineBuffer::new(100);
        for c in "3 + 5".chars() {
            buf.insert_char(c);
        }
        // Cursor at end: "3 + 5|"

        // Delete word left - should delete '5'
        buf.delete_word_left();
        assert_eq!(buf.as_str().unwrap(), "3 + ");

        // Delete word left - should delete '+'
        buf.delete_word_left();
        assert_eq!(buf.as_str().unwrap(), "3 ");
    }

    // History tests
    #[test]
    fn test_history_add() {
        let mut hist = History::new(10);
        hist.add("first");
        hist.add("second");

        assert_eq!(hist.previous(""), Some("second"));
        assert_eq!(hist.previous(""), Some("first"));
        assert_eq!(hist.previous(""), None); // no more
    }

    #[test]
    fn test_history_skip_empty() {
        let mut hist = History::new(10);
        hist.add("first");
        hist.add("");
        hist.add("second");

        assert_eq!(hist.previous(""), Some("second"));
        assert_eq!(hist.previous(""), Some("first"));
        assert_eq!(hist.previous(""), None);
    }

    #[test]
    fn test_history_skip_duplicates() {
        let mut hist = History::new(10);
        hist.add("test");
        hist.add("test"); // should be skipped
        hist.add("other");

        assert_eq!(hist.previous(""), Some("other"));
        assert_eq!(hist.previous(""), Some("test"));
        assert_eq!(hist.previous(""), None);
    }

    #[test]
    fn test_history_navigation() {
        let mut hist = History::new(10);
        hist.add("first");
        hist.add("second");
        hist.add("third");

        // Go back through history
        assert_eq!(hist.previous(""), Some("third"));
        assert_eq!(hist.previous(""), Some("second"));

        // Go forward
        assert_eq!(hist.next_entry(), Some("third"));
        assert_eq!(hist.next_entry(), Some("")); // returns saved line (empty string)
    }

    #[test]
    fn test_history_saves_current_line() {
        let mut hist = History::new(10);
        hist.add("first");
        hist.add("second");

        // Start typing something
        assert_eq!(hist.previous("hello"), Some("second"));
        assert_eq!(hist.previous("hello"), Some("first"));

        // Navigate back forward
        assert_eq!(hist.next_entry(), Some("second"));
        assert_eq!(hist.next_entry(), Some("hello")); // restored!
    }

    #[test]
    fn test_history_down_without_up() {
        let mut hist = History::new(10);
        hist.add("first");

        // Down without going up first should do nothing
        assert_eq!(hist.next_entry(), None);
    }

    #[test]
    fn test_history_circular_buffer() {
        let mut hist = History::new(3);
        hist.add("first");
        hist.add("second");
        hist.add("third");
        hist.add("fourth"); // overwrites "first"

        assert_eq!(hist.previous(""), Some("fourth"));
        assert_eq!(hist.previous(""), Some("third"));
        assert_eq!(hist.previous(""), Some("second"));
        assert_eq!(hist.previous(""), None); // "first" was overwritten
    }

    #[test]
    fn test_history_reset_view() {
        let mut hist = History::new(10);
        hist.add("first");
        hist.add("second");

        assert_eq!(hist.previous(""), Some("second"));
        hist.reset_view();

        // After reset, previous() should start from most recent again
        assert_eq!(hist.previous(""), Some("second"));
    }

    #[test]
    fn test_line_buffer_utf8() {
        let mut buf = LineBuffer::new(100);
        buf.insert_char('ä');
        buf.insert_char('ö');
        buf.insert_char('ü');
        assert_eq!(buf.as_str().unwrap(), "äöü");
        assert_eq!(buf.len(), 6); // UTF-8 bytes
    }

    #[test]
    fn test_line_buffer_load() {
        let mut buf = LineBuffer::new(100);
        buf.insert_char('x');
        buf.load("hello world");
        assert_eq!(buf.as_str().unwrap(), "hello world");
        assert_eq!(buf.cursor_pos(), 11);
    }
}
