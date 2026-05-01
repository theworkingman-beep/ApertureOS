//! x86_64 HAL stubs
use core::arch::asm;

pub fn init() {
    // TODO: CPU feature detection, APIC setup, IOAPIC, etc.
}

pub fn rdtsc() -> u64 {
    unsafe {
        let mut low: u32 = 0;
        let mut high: u32 = 0;
        asm!(
            "rdtsc",
            lateout("eax") low,
            lateout("edx") high,
            options(nomem, nostack, preserves_flags),
        );
        (high as u64) << 32 | (low as u64)
    }
}

pub fn monotonic_ticks() -> u64 {
    rdtsc()
}

pub fn halt() {
    unsafe { asm!("hlt", options(nomem, nostack)); }
}

pub fn eoi() {
    // TODO: write to LAPIC EOI register
}
