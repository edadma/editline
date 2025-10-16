//! Platform-specific terminal implementations.
//!
//! This module provides [`Terminal`](crate::Terminal) trait implementations for different platforms:
//!
//! - **Unix/Linux**: [`StdioTerminal`] using termios and ANSI escape codes
//! - **Windows**: [`StdioTerminal`] using the Windows Console API
//! - **micro:bit v2**: [`UarteTerminal`] for UART-based serial communication
//!
//! Each implementation handles platform-specific details like raw mode setup,
//! key event parsing, and cursor control.

#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub use unix::StdioTerminal;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::StdioTerminal;

#[cfg(feature = "microbit")]
pub mod microbit;

#[cfg(feature = "microbit")]
pub use microbit::UarteTerminal;
