//! ESP32-S3 REPL example using editline.
//!
//! This example demonstrates how to use the editline library on an ESP32-S3
//! with USB Serial/JTAG for interactive line editing with history.
//!
//! # Hardware
//!
//! - ESP32-S3 development board with USB support
//!
//! # Setup
//!
//! 1. Build: `cargo build --example esp32_repl --features esp32 --target xtensa-esp32s3-espidf`
//! 2. Flash: Use espflash or appropriate flashing tool for your board
//! 3. Connect: Use a serial terminal (picocom, minicom, etc.) to the USB Serial/JTAG port

use editline::{LineEditor, terminals::esp32::UsbSerialJtagTerminal};
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::sys::{
    usb_serial_jtag_driver_config_t,
    usb_serial_jtag_driver_install,
    usb_serial_jtag_is_connected,
    usb_serial_jtag_read_bytes,
};
use std::ffi::c_void;

fn main() {
    // Initialize ESP-IDF
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Starting ESP32-S3 Line Editor REPL on USB Serial/JTAG");

    // Install USB Serial/JTAG driver
    let mut config = usb_serial_jtag_driver_config_t {
        tx_buffer_size: 512,
        rx_buffer_size: 512,
    };

    unsafe {
        usb_serial_jtag_driver_install(&mut config);
    }

    log::info!("USB Serial/JTAG initialized. Waiting for terminal connection...");

    // Wait for a terminal to connect and user to press a key
    let mut input_buffer = [0u8; 1];
    loop {
        let connected = unsafe { usb_serial_jtag_is_connected() };
        if connected {
            // Check if user has sent any input
            let bytes_read = unsafe {
                usb_serial_jtag_read_bytes(input_buffer.as_mut_ptr() as *mut c_void, 1, 0)
            };
            if bytes_read > 0 {
                break;
            }
        }
        FreeRtos::delay_ms(100);
    }

    log::info!("Terminal ready! Starting REPL...");

    // Disable logging to prevent interference with REPL
    unsafe {
        esp_idf_svc::sys::esp_log_level_set(
            b"*\0".as_ptr(),
            esp_idf_svc::sys::esp_log_level_t_ESP_LOG_NONE,
        );
    }

    // Wait for any final log messages to be written and flushed
    FreeRtos::delay_ms(100);

    // Now flush everything from the input buffer
    let mut flush_buffer = [0u8; 256];
    loop {
        let bytes_read = unsafe {
            usb_serial_jtag_read_bytes(flush_buffer.as_mut_ptr() as *mut c_void, flush_buffer.len() as u32, 0)
        };
        if bytes_read == 0 {
            break;
        }
    }

    // Create the terminal and line editor
    let terminal = UsbSerialJtagTerminal::new();
    let mut editor = LineEditor::new(terminal);

    // Show the banner
    editor.terminal_mut().write(b"\r\n========================================\r\n").unwrap();
    editor.terminal_mut().write(b"ESP32-S3 Line Editor REPL\r\n").unwrap();
    editor.terminal_mut().write(b"========================================\r\n").unwrap();
    editor.terminal_mut().write(b"Features:\r\n").unwrap();
    editor.terminal_mut().write(b"- Full line editing with cursor movement\r\n").unwrap();
    editor.terminal_mut().write(b"- Command history (up/down arrows)\r\n").unwrap();
    editor.terminal_mut().write(b"- Word navigation (Ctrl+Left/Right)\r\n").unwrap();
    editor.terminal_mut().write(b"- Delete word (Ctrl+Delete, Alt+Backspace)\r\n").unwrap();
    editor.terminal_mut().write(b"\r\nType 'help' for more info, 'exit' to quit.\r\n").unwrap();
    editor.terminal_mut().write(b"========================================\r\n\r\n").unwrap();

    loop {
        // Read a line from the user
        match editor.read_line("> ") {
            Ok(line) => {
                // Trim whitespace
                let trimmed = line.trim();

                // Skip empty lines
                if trimmed.is_empty() {
                    continue;
                }

                // Handle commands
                match trimmed {
                    "exit" | "quit" => {
                        editor.terminal_mut().write(b"\r\nGoodbye!\r\n").unwrap();
                        break;
                    }
                    "help" => {
                        editor.terminal_mut().write(b"\r\nAvailable commands:\r\n").unwrap();
                        editor.terminal_mut().write(b"  help     - Show this help message\r\n").unwrap();
                        editor.terminal_mut().write(b"  history  - Show command history\r\n").unwrap();
                        editor.terminal_mut().write(b"  clear    - Clear the screen (not implemented)\r\n").unwrap();
                        editor.terminal_mut().write(b"  exit     - Exit the REPL\r\n").unwrap();
                        editor.terminal_mut().write(b"\r\nKeyboard shortcuts:\r\n").unwrap();
                        editor.terminal_mut().write(b"  Up/Down       - Navigate history\r\n").unwrap();
                        editor.terminal_mut().write(b"  Left/Right    - Move cursor\r\n").unwrap();
                        editor.terminal_mut().write(b"  Home/End      - Jump to start/end\r\n").unwrap();
                        editor.terminal_mut().write(b"  Ctrl+Left     - Previous word\r\n").unwrap();
                        editor.terminal_mut().write(b"  Ctrl+Right    - Next word\r\n").unwrap();
                        editor.terminal_mut().write(b"  Backspace     - Delete previous char\r\n").unwrap();
                        editor.terminal_mut().write(b"  Delete        - Delete current char\r\n").unwrap();
                        editor.terminal_mut().write(b"  Ctrl+Delete   - Delete word forward\r\n").unwrap();
                        editor.terminal_mut().write(b"  Alt+Backspace - Delete word backward\r\n").unwrap();
                    }
                    "history" => {
                        editor.terminal_mut().write(b"\r\nCommand history:\r\n").unwrap();
                        let history = editor.get_history();
                        if history.is_empty() {
                            editor.terminal_mut().write(b"  (empty)\r\n").unwrap();
                        } else {
                            for (i, entry) in history.iter().enumerate() {
                                let line_num = format!("  {}: {}\r\n", i + 1, entry);
                                editor.terminal_mut().write(line_num.as_bytes()).unwrap();
                            }
                        }
                    }
                    "clear" => {
                        editor.terminal_mut().write(b"\r\nClear screen not yet implemented.\r\n").unwrap();
                    }
                    _ => {
                        // Echo back what was typed
                        let response = format!("\r\nYou typed: {}\r\n", trimmed);
                        editor.terminal_mut().write(response.as_bytes()).unwrap();
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("\r\nError reading line: {:?}\r\n", e);
                editor.terminal_mut().write(error_msg.as_bytes()).unwrap();
                break;
            }
        }
    }
}
