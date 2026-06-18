//! Minimal cooperative thread scheduler for Windows threads.
//!
//! A single-core round-robin scheduler that switches between the kernel idle
//! context and ready Windows threads. Threads are stored in a static array,
//! and the ready queue holds indices into that array.

use super::thread::{Thread, ThreadState};
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

const MAX_THREADS: usize = 16;
const MAX_READY: usize = MAX_THREADS;

// Static, pinned storage for thread control blocks. The usage bitmap is
// protected by a separate Mutex so we can hand out &'static mut references.
static mut THREAD_STORAGE: [MaybeUninit<Thread>; MAX_THREADS] =
    [const { MaybeUninit::uninit() }; MAX_THREADS];
static THREAD_USED: Mutex<[bool; MAX_THREADS]> = Mutex::new([false; MAX_THREADS]);

static READY_QUEUE: Mutex<[Option<usize>; MAX_READY]> = Mutex::new([const { None }; MAX_READY]);
static CURRENT_THREAD: Mutex<Option<usize>> = Mutex::new(None);
static mut IDLE_RSP: u64 = 0;
static NEXT_TID: AtomicU64 = AtomicU64::new(1);

/// Return the index of a free thread slot.
fn alloc_thread_slot() -> Option<usize> {
    let used = THREAD_USED.lock();
    used.iter().position(|&in_use| !in_use)
}

/// Create a new ready thread starting at `entry_point` with a freshly
/// allocated stack.
pub fn create_thread(pid: u64, entry_point: u64) -> Option<usize> {
    let slot = alloc_thread_slot()?;
    let stack_size = crate::arch::context_switch::stack_size();
    let stack_base = crate::mm::alloc_early(stack_size, 16)? as u64;
    let stack_top = stack_base + stack_size as u64;
    let initial_rsp = crate::arch::context_switch::initial_stack(entry_point, stack_top);

    let tid = NEXT_TID.fetch_add(1, Ordering::Relaxed);
    let mut thread = Thread::new(tid, pid, entry_point);
    thread.stack_base = stack_base;
    thread.stack_limit = stack_base;
    thread.rsp = initial_rsp;
    thread.state = ThreadState::Ready;

    unsafe {
        THREAD_STORAGE[slot].write(thread);
    }
    {
        let mut used = THREAD_USED.lock();
        used[slot] = true;
    }
    enqueue_thread(slot)?;
    Some(slot)
}

/// Add `slot` to the tail of the ready queue.
fn enqueue_thread(slot: usize) -> Option<()> {
    let mut ready = READY_QUEUE.lock();
    let index = ready.iter().position(|s| s.is_none())?;
    ready[index] = Some(slot);
    Some(())
}

/// Return the next ready thread slot, or `None` if the queue is empty.
fn dequeue_thread() -> Option<usize> {
    let mut ready = READY_QUEUE.lock();
    let head = ready.iter().position(|s| s.is_some())?;
    let slot = ready[head].take()?;
    // Shift remaining entries down to keep the queue compact.
    for i in head..MAX_READY - 1 {
        ready[i] = ready[i + 1];
    }
    ready[MAX_READY - 1] = None;
    Some(slot)
}

/// Return an immutable reference to the thread in `slot`.
pub fn thread(slot: usize) -> Option<&'static Thread> {
    if slot >= MAX_THREADS {
        return None;
    }
    unsafe { Some(&*THREAD_STORAGE[slot].as_ptr()) }
}

/// Return a mutable reference to the thread in `slot`.
fn thread_mut(slot: usize) -> Option<&'static mut Thread> {
    if slot >= MAX_THREADS {
        return None;
    }
    // SAFETY: THREAD_STORAGE is a static array of pinned MaybeUninit cells.
    unsafe { Some(&mut *THREAD_STORAGE[slot].as_mut_ptr()) }
}

/// Switch to the next ready thread, saving the current context.
///
/// # Safety
/// Must be called with interrupts disabled or from a state where the scheduler
/// data structures will not be re-entered. This function does not return when
/// switching away; it returns when another thread switches back to the caller.
pub unsafe fn schedule() {
    let Some(next_slot) = dequeue_thread() else {
        return;
    };
    let Some(next) = thread_mut(next_slot) else {
        return;
    };

    let new_rsp = next.rsp;
    next.state = ThreadState::Running;

    let current_slot = *CURRENT_THREAD.lock();
    let old_rsp: *mut u64 = match current_slot {
        Some(slot) => {
            let cur = thread_mut(slot).expect("current thread missing");
            if cur.state != ThreadState::Exited {
                cur.state = ThreadState::Ready;
                let _ = enqueue_thread(slot);
            }
            &mut cur.rsp
        }
        None => core::ptr::addr_of_mut!(IDLE_RSP),
    };

    *CURRENT_THREAD.lock() = Some(next_slot);

    crate::arch::context_switch::switch(old_rsp, new_rsp);
}

/// Entry point placed on every new thread stack. Called if a thread function
/// ever returns.
#[no_mangle]
pub extern "C" fn thread_exit() -> ! {
    {
        if let Some(slot) = *CURRENT_THREAD.lock() {
            if let Some(cur) = thread_mut(slot) {
                cur.state = ThreadState::Exited;
            }
        }
    }
    unsafe { schedule() };
    crate::hlt();
}
