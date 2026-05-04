//! Vibe Coded OS — Kernel Entry Point
#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;

pub use common::{BootInfo, FramebufferInfo, MemoryRegion, MemoryRegionKind};

mod arch;
mod mm;
mod scheduler;
mod syscalls;
mod ipc;
mod shm;
mod compat;
mod drivers;
mod userland;

#[cfg(target_arch = "x86_64")]
use arch::x86_64 as arch_impl;

#[cfg(target_arch = "aarch64")]
use arch::aarch64 as arch_impl;

/// GUI init task — draws the macOS-like desktop directly to framebuffer
extern "C" fn gui_task() -> ! {
    log::info!("gui_task: starting desktop compositor");
    drivers::fbcon::clear(0x1E1E1E); // dark background
    // Draw top bar
    drivers::fbcon::draw_rect(0, 0, 1024, 28, 0x2D2D2D);
    // Draw dock area
    let fb_h = unsafe { drivers::fbcon::fb_height() };
    drivers::fbcon::draw_rect(0, fb_h - 60, 1024, 60, 0x2A2A2A);
    log::info!("gui_task: desktop rendered");
    // Wait for IPC messages from shell/app
    loop {
        scheduler::yield_cpu();
    }
}

/// Shell task — interactive terminal
extern "C" fn shell_task() -> ! {
    log::info!("shell_task: starting");
    userland::shell::init();
    userland::shell::Shell::run();
}

#[no_mangle]
pub extern "C" fn kernel_main(boot_info: *mut BootInfo) -> ! {
    if boot_info.is_null() {
        loop {
            #[cfg(target_arch = "x86_64")]
            unsafe { core::arch::asm!("hlt"); }
            #[cfg(target_arch = "aarch64")]
            unsafe { core::arch::asm!("wfe"); }
        }
    }
    let bi = unsafe { &*boot_info };
    let mem_map = if bi.memory_map_ptr.is_null() || bi.memory_map_len == 0 {
        &[] as &[MemoryRegion]
    } else {
        unsafe { core::slice::from_raw_parts(bi.memory_map_ptr, bi.memory_map_len) }
    };

    drivers::uart::init();
    drivers::uart_logger::init();
    log::info!("kernel_main entered");

    if !bi.framebuffer.is_null() {
        unsafe { drivers::fbcon::init(&*bi.framebuffer); }
        log::info!("framebuffer initialized: {}x{}", unsafe { drivers::fbcon::fb_width() }, unsafe { drivers::fbcon::fb_height() });
    }

    arch_impl::init(unsafe { &mut *boot_info });
    mm::init(mem_map);
    scheduler::init();
    ipc::init();
    shm::init();
    compat::init();
    userland::init();
    syscalls::init();

    log::info!("Spawning GUI and shell tasks.");
    scheduler::spawn(gui_task);
    scheduler::spawn(shell_task);

    scheduler::run_scheduler();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("KERNEL PANIC: {}", info);
    arch_impl::halt_loop();
}
