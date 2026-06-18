//! NT thread abstraction for Windows binaries.

/// Lifecycle state of a thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadState {
    Ready,
    Running,
    Blocked,
    Exited,
}

/// A Windows thread inside a process.
#[derive(Clone, Copy)]
pub struct Thread {
    pub tid: u64,
    pub pid: u64,
    pub entry_point: u64,
    /// Kernel stack base and limit (used for scheduler context).
    pub stack_base: u64,
    pub stack_limit: u64,
    /// Kernel RSP used by the cooperative context switch.
    pub rsp: u64,
    /// User-mode stack pointer. For native threads this is the initial RSP
    /// passed to sysret/iret; for interpreted threads it is unused.
    pub user_rsp: u64,
    /// User-mode instruction pointer. Cached from entry_point for clarity.
    pub user_rip: u64,
    /// Physical address of the owning process's top-level page table (CR3).
    pub process_page_table_root: u64,
    pub state: ThreadState,
}

impl Thread {
    pub fn new(tid: u64, pid: u64, entry_point: u64) -> Self {
        Self {
            tid,
            pid,
            entry_point,
            stack_base: 0,
            stack_limit: 0,
            rsp: 0,
            user_rsp: 0,
            user_rip: entry_point,
            process_page_table_root: 0,
            state: ThreadState::Ready,
        }
    }
}

unsafe impl Send for Thread {}
