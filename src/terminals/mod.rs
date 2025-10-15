// Terminal implementations for different platforms

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
