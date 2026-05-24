//! Desktop Shell for VibeOS
//! Manages focus policy, dock, top bar decorations.
#![no_std]
#![no_main]

extern crate libvibe;

use libvibe::*;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Desktop Shell: communicate with WindowServer via IPC
    // v0: idle loop, receive IPC messages
    loop {
        let mut msg = [0u8; IPC_PAYLOAD_SIZE];
        let result = ipc_recv(&mut msg);
        if result != 0 {
            // Process messages from WindowServer
            // Future: handle dock actions, app launching, etc.
        }
        yield_cpu();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}