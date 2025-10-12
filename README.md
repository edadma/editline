# editline

A platform-agnostic line editor library for Rust with full editing capabilities, command history, and cross-platform terminal support.

[![Crates.io](https://img.shields.io/crates/v/editline.svg)](https://crates.io/crates/editline)
[![Documentation](https://docs.rs/editline/badge.svg)](https://docs.rs/editline)
[![License](https://img.shields.io/badge/license-MIT%2FUnlicense-blue.svg)](LICENSE)

## Overview

`editline` provides a powerful, flexible line editing library with a clean separation between I/O and editing logic. Unlike traditional readline implementations that are tightly coupled to specific terminal APIs, `editline` uses a trait-based design that works with any byte-stream I/O.

**Perfect for:**
- Desktop CLIs and REPLs
- Embedded systems (UART, custom displays)
- Network services (telnet/SSH servers)
- Custom terminal emulators
- Testing with mock I/O

## Why editline?

- **Platform-agnostic core** - editing logic has zero I/O dependencies
- **No global state** - create multiple independent editors
- **Type-safe** - Rust enums and Result types throughout
- **Memory-safe** - no manual memory management
- **Full-featured** - history, word navigation, editing operations
- **Cross-platform** - Unix (termios/ANSI) and Windows (Console API) included

## Features

- **Full line editing**: Insert, delete, cursor movement
- **Word-aware navigation**: Ctrl+Left/Right, Alt+Backspace, Ctrl+Delete
- **Command history**: 50-entry circular buffer with up/down navigation
- **Smart history**: Automatically skips duplicates and empty lines
- **Cross-platform**: Unix (termios/ANSI) and Windows (Console API)
- **Zero global state**: All state is explicitly managed
- **Type-safe**: Strong typing with Result-based error handling

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
editline = "0.0.3"
```

### Basic REPL Example

```rust
use editline::terminals::StdioTerminal;
use editline::LineEditor;

fn main() {
    let mut editor = LineEditor::new(1024, 50);  // buffer size, history size
    let mut terminal = StdioTerminal::new();

    loop {
        print!("> ");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();

        match editor.read_line(&mut terminal) {
            Ok(line) => {
                if line == "exit" {
                    break;
                }

                if !line.is_empty() {
                    println!("typed: {}", line);
                }
            }
            Err(e) => {
                // Handle Ctrl-C and Ctrl-D
                match e.kind() {
                    std::io::ErrorKind::UnexpectedEof => {
                        // Ctrl-D pressed - exit gracefully
                        println!("\nGoodbye!");
                        break;
                    }
                    std::io::ErrorKind::Interrupted => {
                        // Ctrl-C pressed - show message and continue
                        println!("\nInterrupted. Type 'exit' or press Ctrl-D to quit.");
                        continue;
                    }
                    _ => {
                        eprintln!("\nError: {}", e);
                        break;
                    }
                }
            }
        }
    }
}
```

### Custom Terminal Implementation

Implement the `Terminal` trait for your platform:

```rust
use editline::{Terminal, KeyEvent};
use std::io;

struct MyCustomTerminal {
    // Your platform-specific fields
}

impl Terminal for MyCustomTerminal {
    fn read_byte(&mut self) -> io::Result<u8> {
        // Read from your input source
    }

    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        // Write to your output
    }

    fn flush(&mut self) -> io::Result<()> {
        // Flush output
    }

    fn enter_raw_mode(&mut self) -> io::Result<()> {
        // Configure for character-by-character input
    }

    fn exit_raw_mode(&mut self) -> io::Result<()> {
        // Restore normal mode
    }

    fn cursor_left(&mut self) -> io::Result<()> {
        // Move cursor left
    }

    fn cursor_right(&mut self) -> io::Result<()> {
        // Move cursor right
    }

    fn clear_eol(&mut self) -> io::Result<()> {
        // Clear from cursor to end of line
    }

    fn parse_key_event(&mut self) -> io::Result<KeyEvent> {
        // Parse input bytes into key events
    }
}
```

## Running the Example

```bash
cargo run --example simple_repl
```

Try these features:
- Arrow keys for cursor movement
- Home/End keys
- Up/Down for history
- Ctrl+Left/Right for word navigation
- Alt+Backspace to delete word left
- Ctrl+Delete to delete word right
- Ctrl-D to exit (EOF)
- Ctrl-C to interrupt current line (continues REPL)

## Architecture

```
┌───────────────────────────────────────┐
│         LineEditor (lib.rs)           │
│  ┌───────────┐  ┌──────────────────┐  │
│  │LineBuffer │  │ History          │  │
│  │           │  │ (circular buffer)│  │
│  └───────────┘  └──────────────────┘  │
└──────────────────┬────────────────────┘
                   │ Terminal trait
        ┌──────────┴──────────┐
        │                     │
┌───────▼────────┐   ┌────────▼─────────┐
│ Unix Terminal  │   │ Windows Terminal │
│ (termios/ANSI) │   │  (Console API)   │
└────────────────┘   └──────────────────┘
```

## Contributing

Contributions are welcome! Areas for enhancement:
- Tab completion callback hooks
- Multi-line editing support
- Syntax highlighting callbacks
- Additional platform implementations
- More comprehensive tests

## License

Licensed under either of:

- MIT license ([LICENSE](LICENSE))
- The Unlicense ([UNLICENSE](UNLICENSE))

at your option.
