//! AArch64 cooperative context switch (stub).

const STACK_SIZE: usize = 64 * 1024; // 64 KiB kernel stacks

/// Return the default kernel thread stack size.
pub const fn stack_size() -> usize {
    STACK_SIZE
}

/// Switch stacks on AArch64.
///
/// # Safety
/// This is a placeholder; the real implementation saves/restores x19-x29 and
/// LR, then switches SP_EL1.
pub unsafe extern "C" fn switch(_old_rsp: *mut u64, _new_rsp: u64) {
    // Placeholder: on real hardware this performs an AArch64 context switch.
    // For the cross-architecture build stub we simply halt.
    loop {
        core::arch::asm!("wfe", options(nomem, nostack));
    }
}

/// Prepare a new thread stack for AArch64.
pub fn initial_stack(_entry_point: u64, stack_top: u64) -> u64 {
    // Placeholder: return the original top unmodified.
    stack_top
}
