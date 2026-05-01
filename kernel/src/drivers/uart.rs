use core::fmt;

#[cfg(target_arch = "x86_64")]
const COM1_PORT: u16 = 0x3F8;

#[cfg(target_arch = "x86_64")]
fn outb(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val); }
}

#[cfg(target_arch = "x86_64")]
fn inb(port: u16) -> u8 {
    let ret: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") ret, in("dx") port); }
    ret
}

pub fn init() {
    #[cfg(target_arch = "x86_64")]
    {
        outb(COM1_PORT + 1, 0x00);
        outb(COM1_PORT + 3, 0x80);
        outb(COM1_PORT + 0, 0x03);
        outb(COM1_PORT + 1, 0x00);
        outb(COM1_PORT + 3, 0x03);
        outb(COM1_PORT + 2, 0xC7);
        outb(COM1_PORT + 4, 0x0B);
    }
}

fn putc(c: u8) {
    #[cfg(target_arch = "x86_64")]
    {
        while (inb(COM1_PORT + 5) & 0x20) == 0 {}
        outb(COM1_PORT, c);
    }
}

pub fn puts(s: &str) {
    for c in s.bytes() {
        if c == b'\n' { putc(b'\r'); }
        putc(c);
    }
}

pub struct UartWriter;
impl fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        puts(s);
        Ok(())
    }
}
