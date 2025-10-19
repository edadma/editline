# ESP32-S3 Line Editor Example

This example demonstrates how to use the editline library on an ESP32-S3 with USB Serial/JTAG.

## Prerequisites

1. **Rust ESP toolchain**: Install using [espup](https://github.com/esp-rs/espup)
   ```bash
   cargo install espup
   espup install
   source ~/export-esp.sh
   ```

2. **ESP-IDF**: Version 5.3 or later (installed by espup)

3. **Flash tool**: Install espflash
   ```bash
   cargo install espflash
   ```

## Hardware

- ESP32-S3 development board with built-in USB support
- USB cable connected to the USB port (not UART port)

## Building and Running

### Option 1: Using the example in an ESP32 project

Since ESP32 projects require special setup with `cargo generate`, the easiest way to use this example is:

1. Create a new ESP32-S3 project:
   ```bash
   cargo generate esp-rs/esp-idf-template cargo
   ```
   - Select "esp32s3" as the MCU
   - Choose "std" for the template

2. Add editline to your `Cargo.toml`:
   ```toml
   [dependencies]
   editline = { version = "0.0.18", features = ["esp32"] }
   esp-idf-svc = "0.51"
   ```

3. Copy the example code from `examples/esp32_repl.rs` to your `src/main.rs`

4. Build and flash:
   ```bash
   cargo build --release
   cargo espflash flash --monitor
   ```

### Option 2: Using this example directly (advanced)

If you want to build the example directly from the editline repository:

1. Create `.cargo/config.toml` in the editline directory:
   ```toml
   [build]
   target = "xtensa-esp32s3-espidf"

   [target.xtensa-esp32s3-espidf]
   linker = "ldproxy"
   runner = "espflash flash --monitor"
   rustflags = [ "--cfg",  "espidf_time64"]

   [unstable]
   build-std = ["std", "panic_abort"]

   [env]
   MCU="esp32s3"
   ESP_IDF_VERSION = "v5.3.3"
   ```

2. Build the example:
   ```bash
   cargo build --example esp32_repl --features esp32 --release
   ```

3. Flash to your device:
   ```bash
   espflash flash target/xtensa-esp32s3-espidf/release/examples/esp32_repl --monitor
   ```

## Connecting to the REPL

Once flashed, connect to the USB Serial/JTAG port with a terminal emulator:

```bash
# Using picocom
picocom -b 115200 /dev/ttyACM0

# Using screen
screen /dev/ttyACM0 115200

# Using minicom
minicom -D /dev/ttyACM0 -b 115200
```

On macOS, the device will be `/dev/cu.usbmodem*`.
On Windows, it will be a COM port.

Press Enter to start the REPL.

## Features

The REPL includes:

- Full line editing with cursor movement
- Command history (up/down arrows)
- Word navigation (Ctrl+Left/Right)
- Delete word (Ctrl+Delete, Alt+Backspace)
- Built-in commands: `help`, `history`, `exit`

## Troubleshooting

### Build fails with "espflash not found"
```bash
cargo install espflash
```

### Build fails with ESP-IDF errors
Make sure you've sourced the ESP environment:
```bash
source ~/export-esp.sh
```

### Device not found when flashing
- Make sure you're connected to the USB port (not UART)
- Check permissions: `sudo usermod -a -G dialout $USER` (logout/login required)
- Try `espflash board-info` to verify connection

### Nothing happens when connecting
- Wait for the device to boot (a few seconds after flashing)
- Press Enter to trigger the REPL to start
- Check that logging is disabled in the code (it is by default)

## Implementation Details

The ESP32-S3 implementation uses:
- `UsbSerialJtagTerminal` struct implementing the `Terminal` trait
- ESP-IDF's `usb_serial_jtag_*` functions for I/O
- ANSI escape sequences for cursor control (same as Unix/Pico)
- FreeRTOS task delays to avoid busy-waiting when reading input

The implementation is fully asynchronous and integrates with FreeRTOS's task scheduler.
