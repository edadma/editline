# ESP32-S3 Integration Summary

This document summarizes the ESP32-S3 support that has been added to the editline library.

## Files Added

### 1. `src/terminals/esp32.rs`
The main ESP32-S3 terminal implementation:
- **`UsbSerialJtagTerminal` struct**: Implements the `Terminal` trait for ESP32-S3
- **Key features**:
  - Uses ESP-IDF's `usb_serial_jtag_*` functions for I/O
  - Internal read buffer (64 bytes) for efficient byte-by-byte reading
  - Non-blocking reads with FreeRTOS task delays to avoid busy-waiting
  - ANSI escape sequence parsing (same as Unix/Pico implementations)
  - Support for all standard key events (arrows, Ctrl+Left/Right, etc.)

### 2. `examples/esp32_repl.rs`
A complete REPL example for ESP32-S3:
- Shows proper ESP-IDF initialization sequence
- Demonstrates USB Serial/JTAG driver installation
- Waits for terminal connection before starting
- Disables logging to avoid interference with REPL
- Includes built-in commands: help, history, exit
- Full integration with editline's `LineEditor`

### 3. `examples/ESP32_README.md`
Comprehensive documentation for ESP32 users:
- Prerequisites and toolchain setup
- Two deployment methods:
  1. Using the example in a `cargo generate` ESP32 project (recommended)
  2. Building the example directly (advanced)
- Build, flash, and connection instructions
- Troubleshooting guide
- Implementation details

## Files Modified

### 1. `Cargo.toml`
- Added `esp-idf-svc` dependency (target-gated for xtensa/riscv32)
- Added `embuild` build-dependency (target-gated for xtensa/riscv32)
- Added `esp32` feature that enables both dependencies
- Added `esp32_repl` example with `required-features = ["esp32"]`

### 2. `build.rs`
- Added conditional ESP-IDF build support
- Only runs `embuild::espidf::sysenv::output()` when:
  - The `esp32` feature is enabled, AND
  - The target contains "espidf" in its triple

### 3. `src/terminals/mod.rs`
- Added conditional module declaration for `esp32`
- Added public re-export of `UsbSerialJtagTerminal`
- Updated documentation to mention ESP32-S3 support

### 4. `README.md`
- Updated "Perfect for" section to mention ESP32 and RTOS microcontrollers
- Updated Features section to list ESP32-S3 in cross-platform support
- Added ESP32 to dependency examples with proper configuration
- Updated "Supported Platforms" section with ESP32-S3 entry
- Updated "Platform-Specific Behavior" to include ESP32 line endings
- Added build instructions reference to ESP32_README.md

## Implementation Details

### Terminal Trait Implementation
The ESP32 implementation follows the same pattern as the Raspberry Pi Pico USB CDC:
- `read_byte()`: Blocking read with FreeRTOS task delays
- `write()`: Chunked writes with timeout handling
- `flush()`: No-op (write is already synchronous)
- `enter_raw_mode()`/`exit_raw_mode()`: No-op (always raw)
- Cursor control: ANSI escape sequences
- Key parsing: Multi-byte ANSI sequence state machine

### Key Differences from Other Platforms

**vs. Unix/Windows**:
- No terminal mode management needed (no termios/Console API)
- Uses FreeRTOS task delays instead of blocking system calls
- CRLF line endings for serial terminal compatibility

**vs. micro:bit**:
- Uses USB instead of UART
- Faster and more convenient (no external USB-UART adapter needed)
- Built-in USB Serial/JTAG interface

**vs. Raspberry Pi Pico**:
- Uses ESP-IDF's USB Serial/JTAG instead of usb-device crate
- Simpler API (no USB polling required in application code)
- FreeRTOS integration (yielding to scheduler)

### Target Gating Strategy

The ESP32 dependencies are only pulled in when building for actual ESP targets:
```toml
[target.'cfg(any(target_arch = "xtensa", target_arch = "riscv32"))'.dependencies]
esp-idf-svc = { version = "0.51", optional = true }
```

This prevents:
- ESP-IDF build errors when building for desktop
- Unnecessary dependency downloads
- Build failures on systems without ESP toolchain

## Testing

Since ESP32 requires the full ESP-IDF toolchain and target triple, standard tests were run:
- ✓ `cargo build` - compiles on host
- ✓ `cargo test` - all 19 unit tests pass
- ✓ `cargo clippy` - no warnings
- ✓ `cargo build --example simple_repl` - desktop example works

For actual ESP32 testing, users need:
1. ESP32-S3 hardware
2. ESP-IDF toolchain (via espup)
3. espflash tool
4. Proper .cargo/config.toml in their project

## Usage Example

In a `cargo generate esp-rs/esp-idf-template` project:

```toml
[dependencies]
editline = { version = "0.0.18", features = ["esp32"] }
esp-idf-svc = "0.51"
```

```rust
use editline::{LineEditor, terminals::esp32::UsbSerialJtagTerminal};

// Initialize USB Serial/JTAG (see example for full setup)
let terminal = UsbSerialJtagTerminal::new();
let mut editor = LineEditor::new(terminal);

loop {
    match editor.read_line("> ") {
        Ok(line) => println!("You typed: {}", line),
        Err(_) => break,
    }
}
```

## Future Enhancements

Potential improvements:
- Support for other ESP32 variants (ESP32, ESP32-C3, ESP32-C6, etc.)
- UART-based terminal for ESP32 variants without USB
- Integration with esp-println for logging
- Embassy async support (if needed)
- WiFi/Bluetooth terminal implementations
