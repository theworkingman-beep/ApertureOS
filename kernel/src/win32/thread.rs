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
    pub stack_base: u64,
    pub stack_limit: u64,
    pub rsp: u64,
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
            state: ThreadState::Ready,
        }
    }
}

unsafe impl Send for Thread {}
