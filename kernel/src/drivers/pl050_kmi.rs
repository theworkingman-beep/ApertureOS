//! PL050 KMI driver for aarch64 QEMU virt machine
//! Handles PS/2 keyboard (KMI0) and PS/2 mouse (KMI1) via MMIO

use crate::input::{self, InputEvent};
use core::ptr::{read_volatile, write_volatile};
use spin::Mutex;

/// PL050 KMI base addresses (QEMU virt machine)
const KMI0_BASE: *mut u8 = 0x09005000 as *mut u8; // Keyboard
const KMI1_BASE: *mut u8 = 0x09004000 as *mut u8; // Mouse

/// KMI register offsets
const KMI_DATA: usize = 0x00;
const KMI_STATUS: usize = 0x04;
const KMI_CONTROL: usize = 0x08;

/// Status register bits
const KMI_STATUS_RXBUSY: u32 = 0x04; // Receive busy

/// Control register bits
const KMI_CONTROL_ENABLE: u32 = 0x01;   // Enable KMI
const KMI_CONTROL_RXINTEN: u32 = 0x10;  // Receive interrupt enable

/// Mouse state
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseState {
    pub x: u16,
    pub y: u16,
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

/// Keyboard state
#[derive(Debug, Clone, Copy)]
struct KbdState {
    shift: bool,
    caps: bool,
}

static MOUSE_STATE: Mutex<MouseState> = Mutex::new(MouseState {
    x: 400,
    y: 300,
    left: false,
    right: false,
    middle: false,
});
static MOUSE_PHASE: Mutex<u8> = Mutex::new(0);
static MOUSE_BYTE1: Mutex<u8> = Mutex::new(0);
static MOUSE_BYTE2: Mutex<u8> = Mutex::new(0);

static KBD_STATE: Mutex<KbdState> = Mutex::new(KbdState {
    shift: false,
    caps: false,
});

/// Read KMI register
unsafe fn kmi_read(base: *mut u8, offset: usize) -> u32 {
    read_volatile(base.add(offset) as *const u32)
}

/// Write KMI register
unsafe fn kmi_write(base: *mut u8, offset: usize, val: u32) {
    write_volatile(base.add(offset) as *mut u32, val)
}

/// Read KMI data register
unsafe fn kmi_read_data(base: *mut u8) -> u8 {
    read_volatile(base.add(KMI_DATA))
}

/// Initialize KMI0 (keyboard) — caller must ensure device is present
pub fn init_kmi0() {
    unsafe {
        // Disable before configuration
        kmi_write(KMI0_BASE, KMI_CONTROL, 0);
        // Enable with RX interrupt
        kmi_write(KMI0_BASE, KMI_CONTROL, KMI_CONTROL_ENABLE | KMI_CONTROL_RXINTEN);
    }
    log::info!("pl050: KMI0 (keyboard) initialized");
}

/// Initialize KMI1 (mouse) — caller must ensure device is present
pub fn init_kmi1() {
    unsafe {
        // Disable before configuration
        kmi_write(KMI1_BASE, KMI_CONTROL, 0);
        // Enable with RX interrupt
        kmi_write(KMI1_BASE, KMI_CONTROL, KMI_CONTROL_ENABLE | KMI_CONTROL_RXINTEN);
    }
    log::info!("pl050: KMI1 (mouse) initialized");
}

/// Handle KMI0 (keyboard) interrupt
pub fn handle_kmi0_irq() {
    unsafe {
        while (kmi_read(KMI0_BASE, KMI_STATUS) & KMI_STATUS_RXBUSY) == 0 {
            let scancode = kmi_read_data(KMI0_BASE);
            handle_scancode(scancode);
        }
    }
}

/// Handle KMI1 (mouse) interrupt
pub fn handle_kmi1_irq() {
    unsafe {
        while (kmi_read(KMI1_BASE, KMI_STATUS) & KMI_STATUS_RXBUSY) == 0 {
            let byte = kmi_read_data(KMI1_BASE);
            handle_mouse_byte(byte);
        }
    }
}

/// PS/2 scancode to ASCII translation (same as x86_64 ps2kbd)
const SCANCODE_TO_ASCII: &'static [u8] = &[
    0,  27, b'1', b'2', b'3', b'4', b'5', b'6', // 0x00-0x07
    b'7', b'8', b'9', b'0', b'-', b'=', b'\x08', b'\t', // 0x08-0x0F
    b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', // 0x10-0x17
    b'o', b'p', b'[', b']', b'\n', 0, b'a', b's', // 0x18-0x1F
    b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';', // 0x20-0x27
    b'\'', b'`', 0, b'\\', b'z', b'x', b'c', b'v', // 0x28-0x2F
    b'b', b'n', b'm', b',', b'.', b'/', 0, 0, // 0x30-0x37
    0, 0, 0, 0, 0, 0, 0, 0, // 0x38-0x3F
    0, 0, 0, 0, 0, 0, 0, 0, // 0x40-0x47
    0, 0, 0, 0, 0, 0, 0, 0, // 0x48-0x4F
    0, 0, 0, 0, b'7', b'8', b'9', 0, // 0x50-0x57
    b'4', b'5', b'6', 0, b'1', b'2', b'3', b'0', // 0x58-0x5F
];

const SCANCODE_TO_SHIFTED: &'static [u8] = &[
    0,  27, b'!', b'@', b'#', b'$', b'%', b'^', // 0x00-0x07
    b'&', b'*', b'(', b')', b'_', b'+', b'\x08', b'\t', // 0x08-0x0F
    b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', // 0x10-0x17
    b'O', b'P', b'{', b'}', b'\n', 0, b'A', b'S', // 0x18-0x1F
    b'D', b'F', b'G', b'H', b'J', b'K', b'L', b':', // 0x20-0x27
    b'"', b'~', 0, b'|', b'Z', b'X', b'C', b'V', // 0x28-0x2F
    b'B', b'N', b'M', b'<', b'>', b'?', 0, 0, // 0x30-0x37
    0, 0, 0, 0, 0, 0, 0, 0, // 0x38-0x3F
    0, 0, 0, 0, 0, 0, 0, 0, // 0x40-0x47
    0, 0, 0, 0, b'7', b'8', b'9', 0, // 0x48-0x4F
    b'4', b'5', b'6', 0, b'1', b'2', b'3', b'0', // 0x50-0x57
];

fn handle_scancode(scancode: u8) {
    let mut state = KBD_STATE.lock();

    // Key release (0xE0 prefix + key, or key + 0x80)
    if scancode == 0xF0 {
        // Make/break prefix - next byte is key release
        return;
    }

    if scancode == 0x12 || scancode == 0x59 { // Left/Right shift
        state.shift = false;
        return;
    }

    if scancode == 0x58 { // Caps lock
        state.caps = !state.caps;
        return;
    }

    if (scancode as usize) < SCANCODE_TO_ASCII.len() {
        let mut ascii = SCANCODE_TO_ASCII[scancode as usize];

        if state.shift {
            ascii = SCANCODE_TO_SHIFTED[scancode as usize];
        } else if state.caps && ascii >= b'a' && ascii <= b'z' {
            ascii -= 32; // Make uppercase
        }

        if ascii != 0 {
            input::push(InputEvent::KeyPress { ascii });
        }
    }

    drop(state);
}

fn decode_packet(state: &mut MouseState, buttons: u8, dx: i8, dy: i8) {
    state.left = buttons & 0x01 != 0;
    state.right = buttons & 0x02 != 0;
    state.middle = buttons & 0x04 != 0;

    state.x = (state.x as i32 + dx as i32).max(0) as u16;
    state.y = (state.y as i32 - dy as i32).max(0) as u16;
}

fn handle_mouse_byte(byte: u8) {
    let mut phase = MOUSE_PHASE.lock();
    match *phase {
        0 => {
            if byte & 0x08 != 0 {
                *MOUSE_BYTE1.lock() = byte;
                *phase = 1;
            }
        }
        1 => {
            *MOUSE_BYTE2.lock() = byte;
            *phase = 2;
        }
        2 => {
            let buttons = *MOUSE_BYTE1.lock();
            let dx = *MOUSE_BYTE2.lock();
            let dy = byte;

            let mut state = MOUSE_STATE.lock();
            let prev_left = state.left;
            let prev_right = state.right;
            let prev_middle = state.middle;

            decode_packet(&mut state, buttons, dx as i8, dy as i8);

            let x = state.x;
            let y = state.y;
            let left = state.left;
            let right = state.right;
            let middle = state.middle;
            let buttons_byte = if left { 1 } else { 0 }
                | if right { 2 } else { 0 }
                | if middle { 4 } else { 0 };

            drop(state);

            input::push(InputEvent::MouseMove { x, y, buttons: buttons_byte });

            if left && !prev_left {
                input::push(InputEvent::MouseDown { button: 0, x, y });
            } else if !left && prev_left {
                input::push(InputEvent::MouseUp { button: 0, x, y });
            }
            if right && !prev_right {
                input::push(InputEvent::MouseDown { button: 1, x, y });
            } else if !right && prev_right {
                input::push(InputEvent::MouseUp { button: 1, x, y });
            }
            if middle && !prev_middle {
                input::push(InputEvent::MouseDown { button: 2, x, y });
            } else if !middle && prev_middle {
                input::push(InputEvent::MouseUp { button: 2, x, y });
            }

            *phase = 0;
        }
        _ => {
            *phase = 0;
        }
    }
}

pub fn get_mouse_state() -> MouseState {
    *MOUSE_STATE.lock()
}
