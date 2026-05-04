//! Cooperative round-robin scheduler with context switching
use alloc::collections::vec_deque::VecDeque;
use spin::Mutex;

/// Size of a task stack in bytes
pub const STACK_SIZE: usize = 64 * 1024;

/// Saved CPU context for a task
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Context {
    pub rsp: usize,
}

// Safety: Task stack pointers are used only by the scheduler which enforces
// exclusive access. All stack data is zero-initialized.
unsafe impl Send for Task {}

static TASKS: Mutex<VecDeque<Task>> = Mutex::new(VecDeque::new());
static CURRENT_TASK: Mutex<Option<usize>> = Mutex::new(None);
static TASK_COUNTER: Mutex<usize> = Mutex::new(0);

pub struct Task {
    pub id: usize,
    pub stack: *mut u8,
    pub context: Context,
    pub entry: usize,
    pub state: TaskState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Dead,
}

impl Task {
    pub fn new(id: usize, entry: usize) -> Self {
        let stack_layout = alloc::alloc::Layout::from_size_align(STACK_SIZE, 16).unwrap();
        let stack_ptr = unsafe { alloc::alloc::alloc(stack_layout) };
        if stack_ptr.is_null() {
            panic!("Failed to allocate task stack");
        }
        unsafe { core::ptr::write_bytes(stack_ptr, 0, STACK_SIZE); }
        Self {
            id,
            stack: stack_ptr,
            context: Context { rsp: 0 },
            entry,
            state: TaskState::Ready,
        }
    }

    pub fn stack_top(&self) -> usize {
        self.stack as usize + STACK_SIZE
    }

    pub fn init_context(&mut self) {
        // Set up initial stack frame so that restore_context "returns" to entry point
        let mut rsp = self.stack_top();
        rsp -= core::mem::size_of::<usize>();
        // Push entry point as return address
        unsafe {
            *(rsp as *mut usize) = self.entry;
        }
        self.context.rsp = rsp;
    }
}

pub fn init() {
    log::info!("scheduler: initialized");
}

pub fn spawn(entry: extern "C" fn() -> !) -> usize {
    let mut counter = TASK_COUNTER.lock();
    let id = *counter;
    *counter += 1;
    drop(counter);

    let mut task = Task::new(id, entry as usize);
    task.init_context();
    TASKS.lock().push_back(task);
    id
}

pub fn current_task_id() -> usize {
    *CURRENT_TASK.lock().as_ref().unwrap_or(&0)
}

/// Yield CPU to the next task
pub fn yield_cpu() {
    unsafe {
        switch_task();
    }
}

/// Switch to the next ready task
pub unsafe fn switch_task() {
    let mut tasks = TASKS.lock();
    let len = tasks.len();
    if len == 0 {
        return;
    }

    // Find current task index
    let cur_id = *CURRENT_TASK.lock();
    let mut cur_idx = None;
    for (i, t) in tasks.iter().enumerate() {
        if t.id == cur_id.unwrap_or(0) {
            cur_idx = Some(i);
            break;
        }
    }

    // Find next ready task (round-robin)
    let mut next_idx = None;
    for i in 0..len {
        let idx = (cur_idx.unwrap_or(usize::MAX) + 1 + i) % len;
        if tasks[idx].state != TaskState::Dead {
            next_idx = Some(idx);
            break;
        }
    }

    let next_idx = match next_idx {
        Some(n) => n,
        None => return,
    };

    // Save current task context
    if let Some(ci) = cur_idx {
        tasks[ci].context.rsp = save_context(tasks[ci].context.rsp);
    }

    // Mark current task as ready
    if let Some(ci) = cur_idx {
        if tasks[ci].state == TaskState::Running {
            tasks[ci].state = TaskState::Ready;
        }
    }

    // Switch to next task
    let next_id = tasks[next_idx].id;
    let next_rsp = tasks[next_idx].context.rsp;
    tasks[next_idx].state = TaskState::Running;
    *CURRENT_TASK.lock() = Some(next_id);
    drop(tasks);

    restore_context(next_rsp);
}

#[cfg(target_arch = "x86_64")]
unsafe fn save_context(_old_rsp: usize) -> usize {
    // Save callee-saved registers onto the old stack
    let mut rsp = _old_rsp;
    // Push dummy values for r15, r14, r13, r12, rbp, rbx, and entry point
    rsp -= core::mem::size_of::<usize>();
    *(rsp as *mut usize) = 0; // r15
    rsp -= core::mem::size_of::<usize>();
    *(rsp as *mut usize) = 0; // r14
    rsp -= core::mem::size_of::<usize>();
    *(rsp as *mut usize) = 0; // r13
    rsp -= core::mem::size_of::<usize>();
    *(rsp as *mut usize) = 0; // r12
    rsp -= core::mem::size_of::<usize>();
    *(rsp as *mut usize) = 0; // rbp
    rsp -= core::mem::size_of::<usize>();
    *(rsp as *mut usize) = 0; // rbx
    rsp
}

#[cfg(target_arch = "x86_64")]
unsafe fn restore_context(new_rsp: usize) {
    // Restore callee-saved registers and return to the task
    core::arch::asm!(
        "mov rsp, {rsp}",
        "pop rbx",
        "pop rbp",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",
        "ret",
        rsp = in(reg) new_rsp,
        options(nomem, nostack)
    );
}

#[cfg(target_arch = "aarch64")]
unsafe fn save_context(_old_sp: usize) -> usize {
    let mut sp = _old_sp;
    // Push x19-x30 (callee-saved) + entry point
    for _ in 0..13 {
        sp -= core::mem::size_of::<usize>();
        *(sp as *mut usize) = 0;
    }
    sp
}

#[cfg(target_arch = "aarch64")]
unsafe fn restore_context(new_sp: usize) {
    core::arch::asm!(
        "mov sp, {sp}",
        "ldp x19, x20, [sp], #16",
        "ldp x21, x22, [sp], #16",
        "ldp x23, x24, [sp], #16",
        "ldp x25, x26, [sp], #16",
        "ldp x27, x28, [sp], #16",
        "ldp x29, x30, [sp], #16",
        "ret",
        sp = in(reg) new_sp,
        options(nomem, nostack)
    );
}

/// Run the scheduler — starts the first task and never returns
pub fn run_scheduler() -> ! {
    let mut tasks = TASKS.lock();
    let len = tasks.len();
    if len == 0 {
        #[cfg(target_arch = "x86_64")]
        crate::arch::x86_64::halt_loop();
        #[cfg(target_arch = "aarch64")]
        crate::arch::aarch64::halt_loop();
    }

    // Start the first task
    let first_id = tasks[0].id;
    let first_rsp = tasks[0].context.rsp;
    tasks[0].state = TaskState::Running;
    *CURRENT_TASK.lock() = Some(first_id);
    drop(tasks);

    unsafe {
        restore_context(first_rsp);
    }

    // Should never reach here
    loop {
        #[cfg(target_arch = "x86_64")]
        crate::arch::x86_64::halt_loop();
        #[cfg(target_arch = "aarch64")]
        crate::arch::aarch64::halt_loop();
    }
}
