pub mod uart;
pub mod uart_logger;
pub mod fbcon;
#[cfg(target_arch = "x86_64")]
pub mod ps2kbd;
