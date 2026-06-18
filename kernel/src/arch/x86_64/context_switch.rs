//! x86_64 cooperative context switch.
//!
//! The implementation is intentionally minimal: it saves and restores the
//! callee-saved registers (as required by the System V AMD64 ABI) and swaps
//! the stack pointer. A new thread is set up with a fake saved context on its
//! stack so the first switch returns into a small stub that calls the real
//! entry point and, if it ever returns, jumps to `thread_exit`.

use crate::win32::scheduler::thread_exit;

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

/// Naked trampoline used as the initial return address for every new thread.
///
/// It expects the real entry point to be sitting on top of the stack. It pops
/// that address into `rdi`, pushes `thread_exit` as the return address, and
/// `call`s the entry point.
#[cfg(feature = "arch_x86_64")]
#[unsafe(naked)]
unsafe extern "C" fn thread_entry_stub() {
    core::arch::naked_asm!(
        "pop rdi",
        "call rdi",
        "jmp {exit}",
        exit = sym thread_exit,
    );
}

/// Prepare a new thread stack so that its first `switch()` returns to
/// `thread_entry_stub`, which in turn calls `entry_point`.
///
/// `stack_top` is one byte past the highest address of the allocated stack.
pub fn initial_stack(entry_point: u64, stack_top: u64) -> u64 {
    let mut rsp = stack_top;
    // The first value `switch()` will pop after restoring registers is the
    // thread entry stub. Just above it (higher address) we leave the real
    // entry point so the stub can pop it.
    rsp = push_u64(rsp, entry_point); // consumed by thread_entry_stub
    rsp = push_u64(rsp, thread_entry_stub as *const () as u64); // return address for switch()
    rsp = push_u64(rsp, 0); // rbp placeholder
    rsp = push_u64(rsp, 0); // rbx placeholder
    rsp = push_u64(rsp, 0); // r12 placeholder
    rsp = push_u64(rsp, 0); // r13 placeholder
    rsp = push_u64(rsp, 0); // r14 placeholder
    rsp = push_u64(rsp, 0); // r15 placeholder
    rsp
}

fn push_u64(stack_top: u64, value: u64) -> u64 {
    let rsp = stack_top.wrapping_sub(core::mem::size_of::<u64>() as u64);
    unsafe {
        core::ptr::write(rsp as *mut u64, value);
    }
    rsp
}
