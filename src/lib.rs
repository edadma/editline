// Platform-agnostic line editor with history and full editing capabilities
//
// Design: Complete separation of I/O from editing logic via Terminal trait

use std::io;

/// Key events that can be processed by the line editor
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

/// Terminal abstraction - implement this trait for your platform
pub trait Terminal {
    /// Read a single byte from input
    fn read_byte(&mut self) -> io::Result<u8>;

    /// Write bytes to output
    fn write(&mut self, data: &[u8]) -> io::Result<()>;

    /// Flush output
    fn flush(&mut self) -> io::Result<()>;

    /// Enter raw mode (disable line buffering and echo)
    fn enter_raw_mode(&mut self) -> io::Result<()>;

    /// Exit raw mode (restore original terminal settings)
    fn exit_raw_mode(&mut self) -> io::Result<()>;

    /// Move cursor left one position
    fn cursor_left(&mut self) -> io::Result<()>;

    /// Move cursor right one position
    fn cursor_right(&mut self) -> io::Result<()>;

    /// Clear from cursor to end of line
    fn clear_eol(&mut self) -> io::Result<()>;

    /// Parse input bytes into a key event
    fn parse_key_event(&mut self) -> io::Result<KeyEvent>;
}

/// Line buffer for editing
pub struct LineBuffer {
    buffer: Vec<u8>,
    cursor_pos: usize,
}

impl LineBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            cursor_pos: 0,
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor_pos = 0;
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.buffer)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        let mut buf = [0; 4];
        let bytes = c.encode_utf8(&mut buf).as_bytes();

        for &byte in bytes {
            self.buffer.insert(self.cursor_pos, byte);
            self.cursor_pos += 1;
        }
    }

    /// Delete character before cursor (backspace)
    pub fn delete_before_cursor(&mut self) -> bool {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.buffer.remove(self.cursor_pos);
            true
        } else {
            false
        }
    }

    /// Delete character at cursor (delete key)
    pub fn delete_at_cursor(&mut self) -> bool {
        if self.cursor_pos < self.buffer.len() {
            self.buffer.remove(self.cursor_pos);
            true
        } else {
            false
        }
    }

    /// Move cursor left
    pub fn move_cursor_left(&mut self) -> bool {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            true
        } else {
            false
        }
    }

    /// Move cursor right
    pub fn move_cursor_right(&mut self) -> bool {
        if self.cursor_pos < self.buffer.len() {
            self.cursor_pos += 1;
            true
        } else {
            false
        }
    }

    /// Move cursor to start of line
    pub fn move_cursor_to_start(&mut self) -> usize {
        let old_pos = self.cursor_pos;
        self.cursor_pos = 0;
        old_pos
    }

    /// Move cursor to end of line
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

        // If we're on a word char, skip to start of current word
        if pos > 0 && is_word_char(self.buffer[pos - 1]) {
            while pos > 0 && is_word_char(self.buffer[pos - 1]) {
                pos -= 1;
            }
        } else {
            // Skip non-word chars
            while pos > 0 && !is_word_char(self.buffer[pos - 1]) {
                pos -= 1;
            }
            // Then skip word chars
            while pos > 0 && is_word_char(self.buffer[pos - 1]) {
                pos -= 1;
            }
        }

        pos
    }

    /// Find start of word to the right
    fn find_word_start_right(&self) -> usize {
        if self.cursor_pos >= self.buffer.len() {
            return self.buffer.len();
        }

        let mut pos = self.cursor_pos;

        // If on word char, skip to end of current word
        if pos < self.buffer.len() && is_word_char(self.buffer[pos]) {
            while pos < self.buffer.len() && is_word_char(self.buffer[pos]) {
                pos += 1;
            }
        }

        // Skip non-word chars
        while pos < self.buffer.len() && !is_word_char(self.buffer[pos]) {
            pos += 1;
        }

        pos
    }

    /// Move cursor to previous word boundary
    pub fn move_cursor_word_left(&mut self) -> usize {
        let target = self.find_word_start_left();
        let moved = self.cursor_pos - target;
        self.cursor_pos = target;
        moved
    }

    /// Move cursor to next word boundary
    pub fn move_cursor_word_right(&mut self) -> usize {
        let target = self.find_word_start_right();
        let moved = target - self.cursor_pos;
        self.cursor_pos = target;
        moved
    }

    /// Delete word to the left of cursor
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

    /// Delete word to the right of cursor
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

    /// Load text into buffer (for history)
    pub fn load(&mut self, text: &str) {
        self.buffer.clear();
        self.buffer.extend_from_slice(text.as_bytes());
        self.cursor_pos = self.buffer.len();
    }
}

/// Check if a byte is a word character
fn is_word_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Command history with circular buffer
pub struct History {
    entries: Vec<String>,
    capacity: usize,
    current_entry: usize,
    viewing_entry: Option<usize>,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
            current_entry: 0,
            viewing_entry: None,
        }
    }

    /// Add entry to history
    pub fn add(&mut self, line: &str) {
        // Skip empty lines
        if line.is_empty() {
            return;
        }

        // Skip if same as most recent
        if let Some(last) = self.entries.last() {
            if last == line {
                return;
            }
        }

        if self.entries.len() < self.capacity {
            self.entries.push(line.to_string());
            self.current_entry = self.entries.len() - 1;
        } else {
            // Circular buffer - overwrite oldest
            self.current_entry = (self.current_entry + 1) % self.capacity;
            self.entries[self.current_entry] = line.to_string();
        }

        self.viewing_entry = None;
    }

    /// Get previous history entry
    pub fn previous(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }

        match self.viewing_entry {
            None => {
                // First time - start at most recent
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

    /// Get next history entry (moving forward in time)
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
                        self.viewing_entry = None;
                        None
                    }
                } else {
                    // Buffer is full
                    let next = (idx + 1) % self.capacity;
                    if next == (self.current_entry + 1) % self.capacity {
                        self.viewing_entry = None;
                        None
                    } else {
                        self.viewing_entry = Some(next);
                        Some(&self.entries[next])
                    }
                }
            }
        }
    }

    /// Reset history view to current line
    pub fn reset_view(&mut self) {
        self.viewing_entry = None;
    }
}

/// Line editor with history
pub struct LineEditor {
    line: LineBuffer,
    history: History,
}

impl LineEditor {
    pub fn new(buffer_capacity: usize, history_capacity: usize) -> Self {
        Self {
            line: LineBuffer::new(buffer_capacity),
            history: History::new(history_capacity),
        }
    }

    /// Get a line with full editing support
    pub fn read_line<T: Terminal>(&mut self, terminal: &mut T) -> io::Result<String> {
        self.line.clear();
        terminal.enter_raw_mode()?;

        loop {
            let event = terminal.parse_key_event()?;

            if event == KeyEvent::Enter {
                break;
            }

            self.handle_key_event(terminal, event)?;
        }

        terminal.exit_raw_mode()?;
        terminal.write(b"\n")?;
        terminal.flush()?;

        let result = self.line.as_str()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
            .to_string();

        // Add to history if non-empty
        if !result.is_empty() {
            self.history.add(&result);
        }
        self.history.reset_view();

        Ok(result)
    }

    fn handle_key_event<T: Terminal>(&mut self, terminal: &mut T, event: KeyEvent) -> io::Result<()> {
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
                if let Some(text) = self.history.previous() {
                    let text = text.to_string();
                    self.load_history_into_line(terminal, &text)?;
                }
            }
            KeyEvent::Down => {
                if let Some(text) = self.history.next_entry() {
                    let text = text.to_string();
                    self.load_history_into_line(terminal, &text)?;
                } else {
                    self.clear_line_display(terminal)?;
                    self.line.clear();
                }
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

    fn redraw_from_cursor<T: Terminal>(&self, terminal: &mut T) -> io::Result<()> {
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

    fn clear_line_display<T: Terminal>(&self, terminal: &mut T) -> io::Result<()> {
        for _ in 0..self.line.cursor_pos() {
            terminal.cursor_left()?;
        }
        terminal.clear_eol()?;
        Ok(())
    }

    fn load_history_into_line<T: Terminal>(&mut self, terminal: &mut T, text: &str) -> io::Result<()> {
        self.clear_line_display(terminal)?;
        self.line.load(text);
        terminal.write(text.as_bytes())?;
        Ok(())
    }
}

// Re-export terminal implementations
pub mod terminals;
