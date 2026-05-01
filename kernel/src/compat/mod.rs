//! macOS application compatibility layer

pub mod dyld;
pub mod macho;

pub fn init() {
    log::info!("macOS compat layer initialized");
}
