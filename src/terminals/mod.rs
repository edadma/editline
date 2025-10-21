//! Platform-specific terminal implementations.
//!
//! This module provides [`Terminal`](crate::Terminal) trait implementations for different platforms:
//!
//! - **Unix/Linux**: [`StdioTerminal`] using termios and ANSI escape codes
//! - **Windows**: [`StdioTerminal`] using the Windows Console API
//! - **micro:bit v2**: `UarteTerminal` for UART-based serial communication
//! - **Raspberry Pi Pico (RP2040 USB CDC)**: `UsbCdcTerminal` for USB CDC serial communication
//! - **Raspberry Pi Pico 2 (RP2350 USB CDC)**: `UsbCdcTerminal` for USB CDC serial communication
//!
//! Each implementation handles platform-specific details like raw mode setup,
//! key event parsing, and cursor control.

#[cfg(all(unix, feature = "std"))]
mod unix;

#[cfg(all(unix, feature = "std"))]
pub use unix::StdioTerminal;

#[cfg(all(windows, feature = "std"))]
mod windows;

#[cfg(all(windows, feature = "std"))]
pub use windows::StdioTerminal;

#[cfg(feature = "microbit")]
pub mod microbit;

#[cfg(feature = "microbit")]
pub use microbit::UarteTerminal;

#[cfg(feature = "rp_pico_usb")]
pub mod rp_pico_usb;

#[cfg(feature = "rp_pico_usb")]
pub use rp_pico_usb::UsbCdcTerminal;

#[cfg(feature = "rp_pico2_usb")]
pub mod rp_pico2_usb;

#[cfg(feature = "rp_pico2_usb")]
pub use rp_pico2_usb::UsbCdcTerminal;
