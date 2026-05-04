//! PS/2 Keyboard driver for x86_64
//! Uses scancode set 1 (standard PC AT keyboard)
//! IRQ 1 -> IDT entry 33

use spin::Mutex;

static KEYBOARD_BUFFER: Mutex<KeyboardBuffer> = Mutex::new(KeyboardBuffer::new());
static KEYBOARD_HANDLER: Mutex<Option<extern "C" fn(u8)>> = Mutex::new(None);

const KB_BUF_SIZE: usize = 256;

struct KeyboardBuffer {
    buffer: [u8; KB_BUF_SIZE],
    head: usize,
    tail: usize,
}

impl KeyboardBuffer {
    const fn new() -> Self {
        Self {
            buffer: [0; KB_BUF_SIZE],
            head: 0,
            tail: 0,
        }
    }

    fn push(&mut self, byte: u8) {
        let next = (self.head + 1) % KB_BUF_SIZE;
        if next != self.tail {
            self.buffer[self.head] = byte;
            self.head = next;
        }
    }

    fn pop(&mut self) -> Option<u8> {
        if self.head == self.tail {
            None
        } else {
            let byte = self.buffer[self.tail];
            self.tail = (self.tail + 1) % KB_BUF_SIZE;
            Some(byte)
        }
    }
}

/// PS/2 scancode set 1 translation table (unshifted)
const SCANCODE_TABLE: &'static [u8] = &[
    0,  27, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', b'\x08', b'\t', // 0x00-0x0F
    b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', b'\n',  0,    b'a',  0,    // 0x10-0x1F
    0,    0,    b'z', b'x', b'c', b'v', b'b', b'n', b'm', b',', b'.', b'/',  0,    0,    0,    0,    // 0x20-0x2F
    b' ', 0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    // 0x30-0x3F
    0,    0,    0,    0,    0,    0,    0,    0,    0,    b'-', 0,    0,    0,    b'+', 0,    0,    // 0x40-0x4F
    0,    0,    0,    0,    0,    b'.', 0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    // 0x50-0x5F
];

/// Shifted scancode table
const SCANCODE_SHIFT_TABLE: &'static [u8] = &[
    0,  27, b'!', b'@', b'#', b'$', b'%', b'^', b'&', b'*', b'(', b')', b'_', b'+', b'\x08', b'\t',
    b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P', b'{', b'}', b'\n',  0,    b'A',  0,
    0,    0,    b'Z', b'X', b'C', b'V', b'B', b'N', b'M', b'<', b'>', b'?',  0,    0,    0,    0,
    b' ', 0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,
    0,    0,    0,    0,    0,    0,    0,    0,    0,    b'_', 0,    0,    0,    b'+', 0,    0,
    0,    0,    0,    0,    0,    b'.', 0,    0,    0,    0,    0,    0,    0,    0,    0,    0,
];

static SHIFT_PRESSED: Mutex<bool> = Mutex::new(false);
static CAPS_LOCK: Mutex<bool> = Mutex::new(false);

/// Initialize PS/2 keyboard
pub fn init() {
    // Wait for keyboard controller to be ready
    wait_kb_ready();

    // Enable keyboard interface (IRQ 1)
    unsafe {
        // Send command 0xAE: Enable keyboard interface
        outb(0x64, 0xAE);
        // Send command 0x60: Write command byte
        outb(0x64, 0x60);
        wait_kb_ready();
        // Read current command byte
        let mut cmd = inb(0x60);
        // Enable IRQ 1 (bit 0) and keyboard (bit 4)
        cmd |= 0x01;
        cmd &= !0x10;
        outb(0x64, 0x60);
        wait_kb_ready();
        outb(0x60, cmd);
    }

    // Remap PIC: keyboard is IRQ 1 -> interrupt 33 (0x21)
    remap_pic();

    // Unmask IRQ 1 (keyboard) in PIC1
    unsafe {
        let mask = inb(0x21);
        outb(0x21, mask & !0x02); // bit 1 = IRQ 1
    }

    log::info!("ps2kbd: initialized (IRQ 1 -> int 0x21)");
}

/// Read a character from the keyboard buffer (non-blocking)
pub fn read_char() -> Option<u8> {
    KEYBOARD_BUFFER.lock().pop()
}

/// Read a character from the keyboard buffer (blocking)
pub fn read_char_blocking() -> u8 {
    loop {
        if let Some(c) = read_char() {
            return c;
        }
        unsafe { core::arch::asm!("hlt") };
    }
}

/// Check if there's a character available
pub fn has_char() -> bool {
    KEYBOARD_BUFFER.lock().head != KEYBOARD_BUFFER.lock().tail
}

/// Register a callback for keyboard events
pub fn set_handler(handler: extern "C" fn(u8)) {
    *KEYBOARD_HANDLER.lock() = Some(handler);
}

/// Called from the IRQ handler when a scancode is received
pub fn handle_scancode(scancode: u8) {
    const KEY_RELEASE: u8 = 0x80;
    const LSHIFT: u8 = 0x2A;
    const RSHIFT: u8 = 0x36;
    const CAPS_KEY: u8 = 0x3A;

    let mut shift = SHIFT_PRESSED.lock();
    let mut caps = CAPS_LOCK.lock();

    // Handle modifier keys
    if scancode == LSHIFT || scancode == RSHIFT {
        *shift = true;
        return;
    }
    if scancode == (LSHIFT | KEY_RELEASE) || scancode == (RSHIFT | KEY_RELEASE) {
        *shift = false;
        return;
    }
    if scancode == CAPS_KEY {
        *caps = !*caps;
        return;
    }

    // Ignore key releases for normal keys
    if scancode & KEY_RELEASE != 0 {
        return;
    }

    // Translate scancode to ASCII
    let ascii = if scancode < SCANCODE_TABLE.len() as u8 {
        let idx = scancode as usize;
        let mut c = SCANCODE_TABLE[idx];

        // Apply shift if pressed
        if *shift {
            if idx < SCANCODE_SHIFT_TABLE.len() {
                c = SCANCODE_SHIFT_TABLE[idx];
            }
        } else if *caps && c >= b'a' && c <= b'z' {
            c = c - 32;
        }

        c
    } else {
        0
    };

    // Call registered handler
    if let Some(handler) = *KEYBOARD_HANDLER.lock() {
        handler(ascii);
    }

    // Push to buffer
    if ascii != 0 {
        KEYBOARD_BUFFER.lock().push(ascii);
    }
}

fn remap_pic() {
    unsafe {
        // ICW1: Init + 4-byte ICW + cascade mode
        outb(0x20, 0x11);
        outb(0xA0, 0x11);
        // ICW2: Vector offsets (PIC1: 0x20, PIC2: 0x28)
        outb(0x21, 0x20);
        outb(0xA1, 0x28);
        // ICW3: Cascade info (PIC2 on IRQ 2)
        outb(0x21, 0x04);
        outb(0xA1, 0x02);
        // ICW4: 8086 mode
        outb(0x21, 0x01);
        outb(0xA1, 0x01);
        // Mask all interrupts initially (except IRQ 1)
        outb(0x21, 0xFC); // 11111100 - unmask IRQ 0 (timer) and IRQ 1 (keyboard)
        outb(0xA1, 0xFF); // mask all on PIC2
    }
}

fn wait_kb_ready() {
    unsafe {
        let mut timeout = 100000u32;
        while timeout > 0 {
            let status = inb(0x64);
            if status & 0x02 == 0 {
                return;
            }
            timeout -= 1;
        }
    }
}

#[inline]
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("outb %al, %dx", in("al") value, in("dx") port);
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let result: u8;
    core::arch::asm!("inb %dx, %al", in("dx") port, out("al") result);
    result
}

/// End-of-interrupt signal to PIC
pub unsafe fn eoi() {
    outb(0x20, 0x20);
}
