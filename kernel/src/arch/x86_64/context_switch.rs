//! x86_64 cooperative context switch.
//!
//! The implementation is intentionally minimal: it saves and restores the
//! callee-saved registers (as required by the System V AMD64 ABI) and swaps
//! the stack pointer. A new thread is set up with a fake saved context on its
//! stack so the first switch returns directly to its entry point.

const STACK_SIZE: usize = 64 * 1024; // 64 KiB kernel stacks

/// Return the default kernel thread stack size.
pub const fn stack_size() -> usize {
    STACK_SIZE
}

/// Switch from the current stack to `new_rsp`, storing the old stack pointer
/// at `old_rsp`.
///
/// # Safety
/// Both pointers must point to valid, owned stacks. This function never
/// returns to its caller; it returns into the thread represented by `new_rsp`.
#[cfg(feature = "arch_x86_64")]
#[unsafe(naked)]
pub unsafe extern "C" fn switch(old_rsp: *mut u64, new_rsp: u64) {
    core::arch::naked_asm!(
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov [rdi], rsp",
        "mov rsp, rsi",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "ret"
    );
}

/// Prepare a new thread stack so that its first `switch()` returns to
/// `entry_point` with a usable stack.
///
/// `stack_top` is one byte past the highest address of the allocated stack.
pub fn initial_stack(entry_point: u64, stack_top: u64) -> u64 {
    // Callee-saved register placeholders; their values are irrelevant for the
    // first context switch but must balance the pops performed by `switch()`.
    let mut rsp = stack_top;
    rsp = push_u64(rsp, entry_point); // return address
    rsp = push_u64(rsp, 0);           // rbp placeholder
    rsp = push_u64(rsp, 0);           // rbx placeholder
    rsp = push_u64(rsp, 0);           // r12 placeholder
    rsp = push_u64(rsp, 0);           // r13 placeholder
    rsp = push_u64(rsp, 0);           // r14 placeholder
    rsp = push_u64(rsp, 0);           // r15 placeholder
    rsp
}

fn push_u64(stack_top: u64, value: u64) -> u64 {
    let rsp = stack_top.wrapping_sub(core::mem::size_of::<u64>() as u64);
    unsafe {
        core::ptr::write(rsp as *mut u64, value);
    }
    rsp
}
