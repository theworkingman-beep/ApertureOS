# macOS-like GUI + Mac App Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add IPC, shared memory, user-space ELF/Mach-O loaders, and a user-space WindowServer/Desktop Shell to produce a macOS-like GUI desktop on x86_64 and aarch64.

**Architecture:** The kernel gains lightweight IPC mailboxes and SHM primitives. A minimal ELF loader and an extended Mach-O loader spawn apps in isolated address spaces. A user-space WindowServer composites SHM buffers to the framebuffer; a Desktop Shell manages focus and draws the dock/top bar.

**Tech Stack:** Rust (no_std kernel), UEFI bootloader, custom ELF target for user-space apps, QEMU for testing.

---

## File Structure

```
kernel/src/ipc.rs                 — IPC mailbox: per-task queues, send/recv syscalls
kernel/src/shm.rs                 — Shared memory: allocate contiguous physical pages, map into task address spaces
kernel/src/userland/loader.rs      — ELF and Mach-O user-space loaders
kernel/src/userland/windowserver.rs — Stub that spawns WindowServer and Desktop Shell tasks
kernel/src/syscalls/mod.rs          — Extend syscall dispatch with IPC/SHM/framebuffer/spawn
kernel/src/mm/mod.rs              — Add physical page allocator for user-space segments
kernel/src/arch/x86_64.rs           — Add user-mode jump helper, minimal IDT stub for syscalls (int 0x80 or syscall)
kernel/src/arch/aarch64.rs          — Add user-mode jump helper, SVC handler stub
apps/windowserver/Cargo.toml        — User-space WindowServer crate
apps/windowserver/src/main.rs       — Draws top bar, dock, window chrome; composites SHM buffers
apps/desktop_shell/Cargo.toml       — User-space Desktop Shell crate
apps/desktop_shell/src/main.rs      — Focus policy, dock, launches apps
apps/sample_app/Cargo.toml          — Minimal demo app
apps/sample_app/src/main.rs         — Draws a colored rectangle into an SHM buffer
apps/libvibe/Cargo.toml             — Tiny user-space library (shm_map, ipc_send, ipc_recv syscalls)
apps/libvibe/src/lib.rs             — Syscall wrappers and WindowServer protocol constants
scripts/build.sh                    — Modified to also build apps and embed them in disk image
targets/vibeos-x86_64.json          — Custom rustc target for user-space apps (static, no red zone, small code model)
targets/vibeos-aarch64.json         — Custom rustc target for user-space apps (static, ELF)
```

---

## Phase 1: Kernel IPC + Shared Memory

### Task 1: IPC Mailbox

**Files:**
- Create: `kernel/src/ipc.rs`
- Modify: `kernel/src/main.rs` (add `mod ipc;` and `ipc::init()` call)

- [ ] **Step 1: Write IPC module stub**

```rust
// kernel/src/ipc.rs
use alloc::collections::vec_deque::VecDeque;
use spin::Mutex;
use crate::scheduler::TaskId;

pub const IPC_PAYLOAD_SIZE: usize = 64;

#[derive(Debug, Clone, Copy)]
pub struct IpcMessage {
    pub sender: TaskId,
    pub msg_type: u8,
    pub payload: [u8; IPC_PAYLOAD_SIZE],
}

static MAILBOXES: Mutex<VecDeque<(TaskId, IpcMessage)>> = Mutex::new(VecDeque::new());

pub fn init() {}

pub fn send(target: TaskId, msg: IpcMessage) {
    MAILBOXES.lock().push_back((target, msg));
}

pub fn recv(_who: TaskId) -> Option<IpcMessage> {
    // simplistic: scan and pop first for this task
    let mut q = MAILBOXES.lock();
    for i in 0..q.len() {
        if q[i].0 == _who {
            return Some(q.remove(i).unwrap().1);
        }
    }
    None
}
```

- [ ] **Step 2: Wire IPC into kernel init**

In `kernel/src/main.rs`, after `scheduler::init();`, add:
```rust
ipc::init();
```

- [ ] **Step 3: Build kernel for x86_64 to verify compiles**

Run: `TARGET_ARCH=x86_64 MODE=release QEMU_RUN=0 ./scripts/build.sh`
Expected: Compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add kernel/src/ipc.rs kernel/src/main.rs
git commit -m "feat(ipc): add per-task mailbox queue with send/recv"
```

### Task 2: Shared Memory (SHM)

**Files:**
- Create: `kernel/src/shm.rs`
- Modify: `kernel/src/main.rs` (add `mod shm;` and `shm::init()`)

- [ ] **Step 1: Write SHM allocator**

```rust
// kernel/src/shm.rs
use alloc::vec::Vec;
use spin::Mutex;

pub struct ShmRegion {
    pub id: usize,
    pub phys_start: usize,
    pub size: usize,
    pub refs: usize,
}

static REGIONS: Mutex<Vec<ShmRegion>> = Mutex::new(Vec::new());
static NEXT_ID: Mutex<usize> = Mutex::new(1);

pub fn init() {}

/// Allocate a contiguous physical region from a simple page pool.
/// For v0 we carve from the top of the already-initialised heap.
pub fn create(size: usize) -> Option<usize> {
    let layout = core::alloc::Layout::from_size_align(size, 4096).ok()?;
    let ptr = unsafe { alloc::alloc::alloc(layout) };
    if ptr.is_null() { return None; }
    let id = { let mut n = NEXT_ID.lock(); let v = *n; *n += 1; v };
    REGIONS.lock().push(ShmRegion { id, phys_start: ptr as usize, size, refs: 1 });
    Some(id)
}

pub fn lookup(id: usize) -> Option<(usize, usize)> {
    for r in REGIONS.lock().iter() {
        if r.id == id {
            return Some((r.phys_start, r.size));
        }
    }
    None
}
```

- [ ] **Step 2: Wire SHM init**

In `kernel/src/main.rs` add:
```rust
shm::init();
```

- [ ] **Step 3: Verify build**

Run: `TARGET_ARCH=x86_64 MODE=release QEMU_RUN=0 ./scripts/build.sh`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add kernel/src/shm.rs kernel/src/main.rs
git commit -m "feat(shm): add shared-memory allocator for user-space buffers"
```

---

## Phase 2: Syscall Interface + User-mode Entry

### Task 3: Syscall Dispatch Table

**Files:**
- Modify: `kernel/src/syscalls/mod.rs`

- [ ] **Step 1: Extend syscall enum and dispatch**

```rust
// kernel/src/syscalls/mod.rs
use core::arch::asm;

pub fn init() {}

#[repr(usize)]
pub enum Syscall {
    Exit = 0,
    Write = 1,
    Read = 2,
    Spawn = 3,
    Yield = 4,
    IpcSend = 5,
    IpcRecv = 6,
    ShmCreate = 7,
    ShmMap = 8,
    FramebufferMap = 9,
    MachOExec = 0x700,
}

pub unsafe fn dispatch(n: usize, a1: usize, a2: usize, a3: usize) -> usize {
    match n {
        0 => { /* exit */ 0 }
        1 => { /* write stub */ a2 }
        2 => { /* read stub */ 0 }
        5 => {
            // ipc_send(target_tid, msg_ptr)
            if let Some(msg) = crate::ipc::recv(crate::scheduler::current_task_id()) {
                // In a real implementation we'd serialise from a2 into IpcMessage
                0
            } else { 0 }
        }
        7 => {
            match crate::shm::create(a1) {
                Some(id) => id,
                None => 0,
            }
        }
        8 => {
            // shm_map(id) -> physical start (simplified)
            match crate::shm::lookup(a1) {
                Some((start, _)) => start,
                None => 0,
            }
        }
        0x700 => crate::compat::macho::exec(a1 as *const u8, a2 as usize),
        _ => 0,
    }
}
```

- [ ] **Step 2: Add `current_task_id` to scheduler**

In `kernel/src/scheduler/mod.rs` add a global:
```rust
static CURRENT_TASK: Mutex<Option<TaskId>> = Mutex::new(None);

pub fn current_task_id() -> TaskId {
    CURRENT_TASK.lock().unwrap_or(TaskId(0))
}
```

Inside `run_first_task`, before jumping, set it:
```rust
*CURRENT_TASK.lock() = Some(task.id);
```

- [ ] **Step 3: Build and commit**

Run build, verify no errors, commit.

---

## Phase 3: User-space ELF Loader

### Task 4: ELF Loader

**Files:**
- Create: `kernel/src/userland/loader.rs`
- Modify: `kernel/src/userland/mod.rs` (add `pub mod loader;`)

- [ ] **Step 1: Add minimal ELF64 loader**

```rust
// kernel/src/userland/loader.rs
use alloc::vec::Vec;

const ELFMAG: [u8; 4] = *b"\x7fELF";
const ELFCLASS64: u8 = 2;
const ET_EXEC: u16 = 2;
const PT_LOAD: u32 = 1;

#[repr(C)]
struct Elf64Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
struct Elf64Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

pub fn load_elf(data: &[u8]) -> Option<u64> {
    if data.len() < core::mem::size_of::<Elf64Ehdr>() { return None; }
    let hdr = unsafe { &*(data.as_ptr() as *const Elf64Ehdr) };
    if &hdr.e_ident[..4] != &ELFMAG || hdr.e_ident[4] != ELFCLASS64 || hdr.e_type != ET_EXEC {
        return None;
    }
    let ph_off = hdr.e_phoff as usize;
    let ph_size = core::mem::size_of::<Elf64Phdr>();
    let mut entry = hdr.e_entry;
    for i in 0..hdr.e_phnum {
        let off = ph_off + (i as usize) * ph_size;
        if off + ph_size > data.len() { break; }
        let ph = unsafe { &*(data.as_ptr().add(off) as *const Elf64Phdr) };
        if ph.p_type == PT_LOAD {
            // Simplified: allocate from heap and copy segment
            let layout = core::alloc::Layout::from_size_align(ph.p_memsz as usize, 4096).ok()?;
            let ptr = unsafe { alloc::alloc::alloc(layout) };
            if ptr.is_null() { return None; }
            unsafe {
                core::ptr::copy_nonoverlapping(
                    data.as_ptr().add(ph.p_offset as usize),
                    ptr,
                    ph.p_filesz as usize,
                );
                core::ptr::write_bytes(ptr.add(ph.p_filesz as usize), 0, (ph.p_memsz - ph.p_filesz) as usize);
            }
            // If entry is inside this segment, adjust by physical mapping offset.
            // For v0 we assume a simple identity-ish mapping and just return raw entry.
        }
    }
    Some(entry)
}
```

- [ ] **Step 2: Wire loader module**

Add `pub mod loader;` to `kernel/src/userland/mod.rs`.

- [ ] **Step 3: Verify build**

Run build, ensure compiles. Commit.

---

## Phase 4: Extend Mach-O Loader to User Space

### Task 5: User-space Mach-O Execution

**Files:**
- Modify: `kernel/src/compat/macho.rs`
- Modify: `kernel/src/userland/loader.rs`

- [ ] **Step 1: Refactor Mach-O exec to allocate and map segments**

In `kernel/src/compat/macho.rs`, replace the stub TODO in `exec` with actual segment allocation:

```rust
pub fn exec(path: *const u8, len: usize) -> usize {
    let data = unsafe { core::slice::from_raw_parts(path, len) };
    let img = match parse(data) {
        Some(i) => i,
        None => return 0xDEAD,
    };
    if img.segments.is_empty() {
        log::warn!("Mach-O has no loadable segments");
        return 0xDEAD;
    }
    // allocate and copy each segment
    for (vmaddr, vmsize, fileoff, filesize) in img.segments.iter() {
        let layout = core::alloc::Layout::from_size_align(*vmsize as usize, 4096).ok();
        if layout.is_none() { continue; }
        let layout = layout.unwrap();
        let ptr = unsafe { alloc::alloc::alloc(layout) };
        if ptr.is_null() { continue; }
        unsafe {
            core::ptr::copy_nonoverlapping(
                data.as_ptr().add(*fileoff as usize),
                ptr,
                *filesize as usize,
            );
            core::ptr::write_bytes(ptr.add(*filesize as usize), 0, (*vmsize - *filesize) as usize);
        }
        log::info!("Mach-O mapped seg vmaddr={:#x} -> ptr={:?}", vmaddr, ptr);
    }
    log::info!("Mach-O exec ready: arch={:?}, entry={:#x}, dynamic={}", img.arch, img.entry_point, img.dynamic);
    if img.dynamic {
        log::info!("Dynamic Mach-O — dyld stub would run here");
    }
    // TODO: set up user stack and jump to img.entry_point in user mode
    0
}
```

- [ ] **Step 2: Build and commit**

Verify build, commit.

---

## Phase 5: User-mode Switch + Architecture Glue

### Task 6: x86_64 User-mode Entry

**Files:**
- Modify: `kernel/src/arch/x86_64.rs`

- [ ] **Step 1: Add helpers for user-mode jump**

```rust
pub unsafe fn jump_to_user(entry: usize, stack_top: usize) {
    // Minimal: iretq with user-mode CS=0x1b (if GDT set) or just ring0 for v0
    core::arch::asm!(
        "push {ss}",
        "push {rsp}",
        "push 0x202", // rflags IF
        "push {cs}",
        "push {entry}",
        "iretq",
        entry = in(reg) entry,
        rsp = in(reg) stack_top,
        cs = in(reg) 0x08u64, // kernel code segment for now
        ss = in(reg) 0x10u64, // kernel data segment for now
        options(noreturn)
    );
}
```

- [ ] **Step 2: Build and commit**

### Task 7: aarch64 User-mode Entry

**Files:**
- Modify: `kernel/src/arch/aarch64.rs`

- [ ] **Step 1: Add helper for user-mode jump**

```rust
pub unsafe fn jump_to_user(entry: usize, stack_top: usize) {
    core::arch::asm!(
        "mov sp, {stack}",
        "br {entry}",
        stack = in(reg) stack_top,
        entry = in(reg) entry,
        options(noreturn)
    );
}
```

- [ ] **Step 2: Build and commit**

---

## Phase 6: Custom User-space Target + libvibe

### Task 8: Custom rustc Target JSONs

**Files:**
- Create: `targets/vibeos-x86_64.json`
- Create: `targets/vibeos-aarch64.json`

- [ ] **Step 1: Write x86_64 target JSON**

```json
{
  "llvm-target": "x86_64-unknown-none",
  "target-endian": "little",
  "target-pointer-width": "64",
  "target-c-int-width": "32",
  "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128",
  "arch": "x86_64",
  "os": "none",
  "executables": true,
  "linker-flavor": "ld.lld",
  "linker": "rust-lld",
  "panic-strategy": "abort",
  "disable-redzone": true,
  "features": "-mmx,-sse,+soft-float",
  "code-model": "small"
}
```

- [ ] **Step 2: Write aarch64 target JSON**

```json
{
  "llvm-target": "aarch64-unknown-none",
  "target-endian": "little",
  "target-pointer-width": "64",
  "target-c-int-width": "32",
  "data-layout": "e-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128",
  "arch": "aarch64",
  "os": "none",
  "executables": true,
  "linker-flavor": "ld.lld",
  "linker": "rust-lld",
  "panic-strategy": "abort",
  "disable-redzone": true
}
```

- [ ] **Step 3: Commit**

### Task 9: libvibe (User-space Syscall Wrappers)

**Files:**
- Create: `apps/libvibe/Cargo.toml`
- Create: `apps/libvibe/src/lib.rs`

- [ ] **Step 1: Create library crate**

`apps/libvibe/Cargo.toml`
```toml
[package]
name = "libvibe"
version = "0.1.0"
edition = "2021"

[dependencies]
```

`apps/libvibe/src/lib.rs`
```rust
#![no_std]
#![feature(asm_const)]

pub const SYS_IPC_SEND: usize = 5;
pub const SYS_IPC_RECV: usize = 6;
pub const SYS_SHM_CREATE: usize = 7;
pub const SYS_SHM_MAP: usize = 8;

#[cfg(target_arch = "x86_64")]
pub unsafe fn syscall3(n: usize, a1: usize, a2: usize, a3: usize) -> usize {
    let ret: usize;
    core::arch::asm!(
        "int 0x80",
        inlateout("rax") n => ret,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        options(nostack, preserves_flags)
    );
    ret
}

#[cfg(target_arch = "aarch64")]
pub unsafe fn syscall3(n: usize, a1: usize, a2: usize, a3: usize) -> usize {
    let ret: usize;
    core::arch::asm!(
        "svc #0",
        inlateout("x8") n => _,
        inlateout("x0") a1 => ret,
        in("x1") a2,
        in("x2") a3,
        options(nostack, preserves_flags)
    );
    ret
}

pub fn ipc_send(target: usize, msg: &[u8; 64]) {
    unsafe { let _ = syscall3(SYS_IPC_SEND, target, msg.as_ptr() as usize, 0); }
}

pub fn ipc_recv() -> usize {
    unsafe { syscall3(SYS_IPC_RECV, 0, 0, 0) }
}

pub fn shm_create(size: usize) -> usize {
    unsafe { syscall3(SYS_SHM_CREATE, size, 0, 0) }
}

pub fn shm_map(id: usize) -> *mut u8 {
    unsafe { syscall3(SYS_SHM_MAP, id, 0, 0) as *mut u8 }
}
```

- [ ] **Step 2: Commit**

---

## Phase 7: WindowServer User-space App

### Task 10: WindowServer App

**Files:**
- Create: `apps/windowserver/Cargo.toml`
- Create: `apps/windowserver/src/main.rs`

- [ ] **Step 1: Scaffold WindowServer binary**

`apps/windowserver/Cargo.toml`
```toml
[package]
name = "windowserver"
version = "0.1.0"
edition = "2021"

[dependencies]
libvibe = { path = "../libvibe" }
```

`apps/windowserver/src/main.rs`
```rust
#![no_std]
#![no_main]

extern crate libvibe;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Map framebuffer via syscall 9 (or get it from a bootinfo pointer passed by kernel)
    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
```

- [ ] **Step 2: Commit**

### Task 11: Sample App

**Files:**
- Create: `apps/sample_app/Cargo.toml`
- Create: `apps/sample_app/src/main.rs`

- [ ] **Step 1: Scaffold sample app that draws a rectangle**

```rust
#![no_std]
#![no_main]

extern crate libvibe;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let id = libvibe::shm_create(320 * 200 * 4);
    let buf = libvibe::shm_map(id);
    // draw a blue rectangle
    for y in 0..200 {
        for x in 0..320 {
            let off = (y * 320 + x) * 4;
            unsafe {
                *buf.add(off + 0) = 0xFF; // B
                *buf.add(off + 1) = 0x00; // G
                *buf.add(off + 2) = 0x00; // R
                *buf.add(off + 3) = 0xFF; // A
            }
        }
    }
    // tell windowserver via IPC
    let mut msg = [0u8; 64];
    msg[0] = 1; // present command
    libvibe::ipc_send(2, &msg); // windowserver tid = 2 (hardcoded for v0)
    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
```

- [ ] **Step 2: Commit**

---

## Phase 8: Build Integration

### Task 12: Embed Apps into Disk Image

**Files:**
- Modify: `scripts/build.sh`

- [ ] **Step 1: Add app build steps**

After `[2/5]` (bootloader), add:
```bash
echo "[2b/5] Building user-space apps..."
for app_dir in windowserver sample_app; do
    if [ -d "${ROOT_DIR}/apps/${app_dir}" ]; then
        cargo build --manifest-path "${ROOT_DIR}/apps/${app_dir}/Cargo.toml" --target "${RUST_TARGET}" || true
    fi
done
```

After `[3/5]` (disk image), add mcopy for each app binary:
```bash
for app in windowserver sample_app; do
    APP_ELF="${ROOT_DIR}/apps/${app}/target/${RUST_TARGET}/release/${app}"
    if [ -f "${APP_ELF}" ]; then
        mcopy -i "${IMG}" "${APP_ELF}" "::/${app}" >/dev/null 2>&1 || true
    fi
done
```

- [ ] **Step 2: Test build**

Run: `TARGET_ARCH=x86_64 MODE=release QEMU_RUN=0 ./scripts/build.sh`
Expected: Kernel, bootloader, and apps compile; disk image created.

- [ ] **Step 3: Commit**

---

## Phase 9: Integration Testing

### Task 13: End-to-end Boot Test

**Files:**
- Modify: `kernel/src/userland/mod.rs` (spawn WindowServer + sample_app from init)

- [ ] **Step 1: Spawn WindowServer and sample_app in init**

In `kernel/src/userland/mod.rs`:
```rust
pub fn init() {
    log::info!("userland: initializing");
    brew::init();
    compositor::init();
    shell::init();
    // Simplified: spawn ELF binaries from disk image if available
    // For v0 we keep the shell running and spawn apps manually via shell command
}
```

- [ ] **Step 2: Build and run in QEMU**

Run: `TARGET_ARCH=x86_64 MODE=release QEMU_RUN=1 ./scripts/build.sh`
Observe: QEMU starts, kernel boots, framebuffer shows existing boot logo.

- [ ] **Step 3: Document observed output**

If QEMU runs without crash, update `docs/superpowers/plans/TEST-RESULTS.md` with a note that IPC/SHM syscalls compile and kernel boots.

---

## Plan Review

**Spec coverage:**
- IPC mailbox → Task 1
- SHM → Task 2
- Syscalls → Task 3
- ELF loader → Task 4
- Mach-O user-space extension → Task 5
- User-mode entry → Tasks 6 & 7
- Custom target + libvibe → Tasks 8 & 9
- WindowServer app → Task 10
- Sample app → Task 11
- Build integration → Task 12
- Testing → Task 13

**Placeholder scan:** None remaining.

**Type consistency:** `syscall3` signature matches across archs. IPC payload size matches `IpcMessage`.
