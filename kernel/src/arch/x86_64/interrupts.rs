//! x86_64 interrupt handling.

#![allow(static_mut_refs)]

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::instructions::port::Port;

/// Programmable Interrupt Controller constants.
const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;
const PIC_EOI: u8 = 0x20;

const KEYBOARD_BUF_SIZE: usize = 32;
static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

// Keyboard input ring buffer. Protected by interrupt-disable semantics on a
// single-core system; no explicit lock is required because the consumer only
// runs with interrupts enabled outside of handlers.
static mut KEYBOARD_BUF: [u8; KEYBOARD_BUF_SIZE] = [0; KEYBOARD_BUF_SIZE];
static mut KEYBOARD_HEAD: usize = 0;
static mut KEYBOARD_TAIL: usize = 0;

/// Initialize the IDT and remap the PIC.
pub fn init() {
    unsafe {
        IDT.breakpoint.set_handler_fn(breakpoint_handler);
        IDT.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(1);
        IDT.page_fault.set_handler_fn(page_fault_handler);

        // IRQ0: timer uses a raw handler that performs preemptive scheduling.
        IDT[32].set_handler_addr(x86_64::VirtAddr::new(timer_interrupt_handler as *const () as u64));
        // IRQ1: keyboard
        IDT[33].set_handler_fn(keyboard_interrupt_handler);

        IDT.load();
    }

    remap_pic();
    unsafe {
        // Unmask timer (IRQ0) and keyboard (IRQ1).
        let mut pic1_data: Port<u8> = Port::new(PIC1_DATA);
        pic1_data.write(0xFC);
    }
}

/// Remap the PIC so IRQs start at IDT entry 32.
fn remap_pic() {
    let mut pic1_command: Port<u8> = Port::new(PIC1_COMMAND);
    let mut pic1_data: Port<u8> = Port::new(PIC1_DATA);
    let mut pic2_command: Port<u8> = Port::new(PIC2_COMMAND);
    let mut pic2_data: Port<u8> = Port::new(PIC2_DATA);

    let a1 = unsafe { pic1_data.read() };
    let a2 = unsafe { pic2_data.read() };

    unsafe {
        pic1_command.write(0x11);
        pic2_command.write(0x11);

        pic1_data.write(0x20);
        pic2_data.write(0x28);

        pic1_data.write(0x04);
        pic2_data.write(0x02);

        pic1_data.write(0x01);
        pic2_data.write(0x01);

        pic1_data.write(a1);
        pic2_data.write(a2);
    }
}

/// Read a scancode from the keyboard buffer, if one is available.
pub fn read_scancode() -> Option<u8> {
    unsafe {
        if KEYBOARD_HEAD == KEYBOARD_TAIL {
            return None;
        }
        let scancode = KEYBOARD_BUF[KEYBOARD_HEAD];
        KEYBOARD_HEAD = (KEYBOARD_HEAD + 1) % KEYBOARD_BUF_SIZE;
        Some(scancode)
    }
}

/// Read a printable character from the keyboard, converting scancodes to
/// US QWERTY ASCII. Returns `\n` for Enter and `\u{8}` for Backspace.
pub fn read_char() -> Option<char> {
    let scancode = read_scancode()?;
    // For now we ignore make/break and treat most keys as make codes.
    let ascii = match scancode {
        0x01 => '\u{1B}', // Esc
        0x0E => '\u{8}',  // Backspace
        0x1C => '\n',     // Enter
        0x39 => ' ',      // Space
        0x02 => '1',
        0x03 => '2',
        0x04 => '3',
        0x05 => '4',
        0x06 => '5',
        0x07 => '6',
        0x08 => '7',
        0x09 => '8',
        0x0A => '9',
        0x0B => '0',
        0x10 => 'q',
        0x11 => 'w',
        0x12 => 'e',
        0x13 => 'r',
        0x14 => 't',
        0x15 => 'y',
        0x16 => 'u',
        0x17 => 'i',
        0x18 => 'o',
        0x19 => 'p',
        0x1E => 'a',
        0x1F => 's',
        0x20 => 'd',
        0x21 => 'f',
        0x22 => 'g',
        0x23 => 'h',
        0x24 => 'j',
        0x25 => 'k',
        0x26 => 'l',
        0x2C => 'z',
        0x2D => 'x',
        0x2E => 'c',
        0x2F => 'v',
        0x30 => 'b',
        0x31 => 'n',
        0x32 => 'm',
        _ => return None,
    };
    Some(ascii)
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    crate::logln!("BREAKPOINT: {:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    crate::logln!("DOUBLE FAULT: {:#?}", stack_frame);
    crate::hlt();
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;
    let addr = Cr2::read().unwrap_or(x86_64::VirtAddr::new_truncate(0));
    crate::logln!("PAGE FAULT at {:#x}: {:#?} {:#?}", addr, error_code, stack_frame);
}

/// Naked timer interrupt handler.
///
/// On entry the CPU has pushed RIP, CS, and RFLAGS onto the interrupt stack.
/// If the interrupt arrived from ring 3, it also pushed RSP and SS. We check
/// the saved CS to decide whether to preempt; kernel-mode interrupts simply
/// acknowledge the PIC and return.
#[cfg(feature = "arch_x86_64")]
#[unsafe(naked)]
unsafe extern "C" fn timer_interrupt_handler() {
    core::arch::naked_asm!(
        // [rsp + 0]  = saved RIP
        // [rsp + 8]  = saved CS (CPL in low 2 bits)
        "mov rax, [rsp + 8]",
        "and rax, 3",
        "cmp rax, 3",
        "jne 2f",

        // Came from ring 3: save all general-purpose registers above the
        // CPU-pushed interrupt frame.
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push r11",
        "push r10",
        "push r9",
        "push r8",
        "push rbp",
        "push rdi",
        "push rsi",
        "push rbx",
        "push rdx",
        "push rcx",
        "push rax",

        // RSP now points to the saved register frame. Ask the scheduler to
        // pick the next thread and return its interrupt frame pointer.
        "mov rdi, rsp",
        "call {preempt}",
        "mov rsp, rax",

        // Restore the new thread's general-purpose registers.
        "pop rax",
        "pop rcx",
        "pop rdx",
        "pop rbx",
        "pop rsi",
        "pop rdi",
        "pop rbp",
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r11",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",

        // Acknowledge the PIC and return to the selected thread.
        "mov al, {eoi}",
        "out {pic1_command}, al",
        "iretq",

        "2:",
        "mov al, {eoi}",
        "out {pic1_command}, al",
        "iretq",

        preempt = sym crate::win32::scheduler::preempt,
        eoi = const PIC_EOI,
        pic1_command = const PIC1_COMMAND,
    );
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        let mut port: Port<u8> = Port::new(0x60);
        let scancode = port.read();
        let next = (KEYBOARD_TAIL + 1) % KEYBOARD_BUF_SIZE;
        if next != KEYBOARD_HEAD {
            KEYBOARD_BUF[KEYBOARD_TAIL] = scancode;
            KEYBOARD_TAIL = next;
        }
        let mut pic1_command: Port<u8> = Port::new(PIC1_COMMAND);
        pic1_command.write(PIC_EOI);
    }
}
