use core::fmt::Write;
use log::{Log, Metadata, Record};

pub struct UartLogger;

impl Log for UartLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        let mut w = super::uart::UartWriter;
        let _ = writeln!(&mut w, "[{}] {}", record.level(), record.args());
    }
    fn flush(&self) {}
}

pub static LOGGER: UartLogger = UartLogger;

pub fn init() {
    super::uart::init();
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Info));
}
