# Vibe Coded OS — macOS-like GUI + Mac App Support Design

**Date:** 2026-05-03  
**Status:** Approved  
**Scope:** Add a macOS-inspired GUI (window server, dock, top bar) and extend existing Mach-O / dyld stubs to run user-space apps on x86_64 and aarch64.

---

## 1. Goals ( ranked )

1. **GUI desktop** — A macOS-like desktop with a top bar, dock, and draggable/resizable windows.
2. **User-space app runtime** — Spawn native apps as isolated tasks with their own address spaces, drawing into shared-memory buffers.
3. **Mach-O / dyld compatibility** — Extend existing Mach-O parser and dyld stub so simple existing macOS CLI binaries can load and execute (best-effort, full AppKit support out of scope).
4. **Multi-arch support** — Everything builds and runs on both x86_64 and aarch64 via UEFI + QEMU.

---

## 2. Architecture

```
┌─────────────────────────────────────────────┐
│  Kernel (no_std Rust, UEFI boot)            │
│  ├── UART logger / frame buffer console     │
│  ├── Memory manager (buddy + slab)          │
│  ├── Scheduler (cooperative / preemptive)   │
│  ├── HAL (x86_64 / aarch64)                 │
│  ├── IPC mailbox (task → task messages)     │
│  ├── Shared memory (SHM) allocator          │
│  ├── User-space loader (ELF + Mach-O)       │
│  └── Syscalls (ipc_send, ipc_recv, shm_create,
│       shm_map, framebuffer_map, spawn, yield)
└─────────────────────────────────────────────┘
                      │
         ┌────────────┼────────────┐
         ▼            ▼            ▼
    ┌─────────┐ ┌─────────────┐ ┌─────────────┐
    │ WindowServer │ Desktop Shell │   Apps      │
    │ (user task)    │ (user task)   │ (user tasks)│
    │ • composites │ • focus policy│ • draw to   │
    │   windows    │ • dock layout │   SHM buffer│
    │ • draws chrome│ • top bar    │ • IPC to WS │
    │ • reads input │              │             │
    └─────────┘ └─────────────┘ └─────────────┘
```

---

## 3. Kernel Extensions

### 3.1 Shared Memory (SHM)

A task calls `shm_create(size)` → kernel allocates contiguous physical pages, returns a handle.  
The task (and any task it passes the handle to) calls `shm_map(handle)` to map the same physical memory into its own address space.

Used for:
- App → WindowServer frame buffers.
- WindowServer → Desktop Shell policy state (optional, IPC is primary).

### 3.2 IPC Mailbox

Per-task mailbox queue of small messages (`type: u8`, `payload: [u8; 64]`).  
Syscalls: `ipc_send(target_tid, msg)`, `ipc_recv(&mut msg)` (blocks if empty).  
Used for input events, window commands, and focus changes.

### 3.3 Framebuffer Mapping

WindowServer has a read-only mapping of the physical framebuffer so it can blit without kernel entry per pixel.

### 3.4 User Task Loader

Extend the existing Mach-O parser in `kernel/src/compat/macho.rs` to:
1. Allocate user pages for each segment.
2. Copy file contents into those pages.
3. Set up a user stack (with fake argv/envp/auxv for Mach-O `LC_MAIN`).
4. Jump to the entry point in user mode.

Also add a minimalELF loader for native VibeOS apps (compiled with a custom target).

---

## 4. WindowServer

Responsibilities:
- Own the framebuffer mapping.
- Maintain a list of windows (position, Z-order, SHM buffer handle).
- Composite windows into the framebuffer each frame (simple blit + alpha).
- Draw global UI: top bar, dock, window chrome.
- Poll mouse/keyboard input from kernel and route events via IPC to the focused app.

Non-goals (for now):
- Hardware acceleration.
- TrueType font rasterization (use bitmap fonts).
- Complex window effects.

---

## 5. Desktop Shell

Responsibilities:
- Decide which window is focused based on user clicks.
- Manage dock icons and launch apps when clicked.
- Update the top bar clock and menu.
- Send window-move/resize commands to WindowServer.

---

## 6. App Runtime

Apps are compiled for VibeOS with a custom target (ELF) or are simple Mach-O binaries.
At runtime each app:
1. Opens a connection to WindowServer (via IPC).
2. Requests a framebuffer SHM buffer of a certain size.
3. Draws pixels normally (any language that can write bytes).
4. Sends an IPC message: “present this buffer at (x, y, z)”.

For Mach-O execution:
- Kernel parses the binary.
- If dynamic, invoke dyld stub to resolve a minimal symbol table.
- Map segments, set up stack, jump to entry.

---

## 7. Error Handling & Safety

- All user-space pointers from IPC are validated before dereference (kernel maps SHM into kernel space for copy if needed).
- WindowServer crashes → kernel reboots or restarts WindowServer (watchdog stub).
- Double-buffer the framebuffer; torn frames are acceptable for v0.

---

## 8. Testing Plan

- Unit tests for the Mach-O parser, ELF loader, and IPC queue in `kernel/`.
- Integration test: build `windowserver` task as a static ELF, boot in QEMU, verify framebuffer shows the top bar and a sample window.
- CI builds both x86_64 and aarch64 and runs the QEMU test.

---

## 9. Files & Modules

| File | Purpose |
|------|---------|
| `kernel/src/ipc.rs` | IPC mailbox implementation |
| `kernel/src/shm.rs` | Shared-memory allocator |
| `kernel/src/userland/loader.rs` | ELF + Mach-O user-space loader |
| `kernel/src/userland/windowserver.rs` | Stub that spawns the WindowServer task |
| `apps/windowserver/` | User-space WindowServer code (Rust, no_std + POSIX-ish libc) |
| `apps/desktop_shell/` | User-space Desktop Shell code |
| `apps/sample_app/` | Minimal demo app that draws a colored rectangle |
| `targets/vibeos-x86_64.json` | Custom rustc target for user-space apps |
| `targets/vibeos-aarch64.json` | Custom rustc target for user-space apps |

---

## 10. Trade-offs Summary

- **User-space vs in-kernel compositor:** Chose user-space for safety and modularity, even though it requires IPC/SHM primitives.
- **ELF vs Mach-O for native apps:** Support both — ELF for simplicity during development, Mach-O for macOS compatibility story.
- **No hardware acceleration yet:** Keeps code portable; virtio-GPU is the future path.

---

*Approved for implementation.*
