#![no_std]
#![no_main]

use bootloader_api::config::Mapping;
use bootloader_api::{entry_point, BootInfo, BootloaderConfig};
use core::fmt::Write;
use core::panic::PanicInfo;
use spin::Mutex;
use uart_16550::SerialPort;

pub static SERIAL1: Mutex<Option<SerialPort>> = Mutex::new(None);

pub fn init_serial() {
    let mut serial = unsafe { SerialPort::new(0x3F8) };
    serial.init();
    *SERIAL1.lock() = Some(serial);
}

pub fn serial_print(args: core::fmt::Arguments) {
    if let Some(serial) = SERIAL1.lock().as_mut() {
        let _ = serial.write_fmt(args);
    }
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config.kernel_stack_size = 64 * 1024; // 64 KiB
    config
};

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    init_serial();

    serial_println!("Aperture OS kernel booting...");
    serial_println!("Bootloader API version: {:?}", boot_info.api_version);

    // Draw a simple color to the framebuffer so we can see the GUI is alive.
    if let Some(fb) = boot_info.framebuffer.as_mut() {
        let info = fb.info();
        let width = info.width;
        let height = info.height;
        let bytes_per_pixel = info.bytes_per_pixel as usize;
        let stride = info.stride;
        let buffer = fb.buffer_mut();

        serial_println!("Framebuffer: {}x{} stride={} bpp={}", width, height, stride, bytes_per_pixel);

        for y in 0..height {
            for x in 0..width {
                let offset = (y * stride + x) * bytes_per_pixel;
                let color = if x < width / 2 && y < height / 2 {
                    [0x00, 0x40, 0x80, 0x00] // teal-ish
                } else if x >= width / 2 && y < height / 2 {
                    [0x40, 0x00, 0x80, 0x00] // purple-ish
                } else if x < width / 2 {
                    [0x80, 0x40, 0x00, 0x00] // orange-ish
                } else {
                    [0x20, 0x20, 0x20, 0x00] // gray
                };
                buffer[offset..offset + bytes_per_pixel]
                    .copy_from_slice(&color[..bytes_per_pixel]);
            }
        }
        serial_println!("Framebuffer initialized.");
    } else {
        serial_println!("No framebuffer available.");
    }

    serial_println!("Kernel idle.");
    loop {
        x86_64::instructions::hlt();
    }
}

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("PANIC: {}", info);
    loop {
        x86_64::instructions::hlt();
    }
}
