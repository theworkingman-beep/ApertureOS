//! Simple timekeeping for gettimeofday syscall

use spin::Mutex;

/// Global uptime counter (incremented by timer tick, in milliseconds)
static UPTIME_MS: Mutex<u64> = Mutex::new(0);

/// Timer frequency in Hz (assumes 1000 Hz timer = 1ms per tick)
const TIMER_HZ: u64 = 1000;

pub fn init() {
    log::info!("time: initialized");
}

/// Called by timer interrupt to increment uptime
pub fn tick() {
    let mut uptime = UPTIME_MS.lock();
    *uptime = uptime.saturating_add(1);
}

/// Get uptime in milliseconds
pub fn uptime_ms() -> u64 {
    *UPTIME_MS.lock()
}

/// Get uptime in seconds and microseconds (for gettimeofday)
pub fn uptime_timeval() -> (u64, u64) {
    let ms = uptime_ms();
    let secs = ms / 1000;
    let usecs = (ms % 1000) * 1000;
    (secs, usecs)
}

/// Get uptime in seconds and nanoseconds (for clock_gettime)
pub fn uptime_timespec() -> (u64, u64) {
    let ms = uptime_ms();
    let secs = ms / 1000;
    let nsecs = (ms % 1000) * 1_000_000;
    (secs, nsecs)
}