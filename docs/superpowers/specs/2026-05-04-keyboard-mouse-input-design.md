# Keyboard + Mouse Input Design — v0.2

## Overview

Add mouse cursor, keyboard input, and GUI event handling to VibeOS. The desktop becomes interactive: move a mouse cursor, click dock icons, drag windows, and type in the shell.

## Architecture

```
PS/2 Mouse IRQ 12 ─┐
                    ├─► kernel/src/input.rs ──► gui_task event loop ──► hit-test ──► render
PS/2 Keyboard IRQ 1 ┘
```

### Components

1. **PS/2 Mouse Driver** (`kernel/src/drivers/ps2mouse.rs`) — x86_64 only, PS/2 protocol
2. **Input Subsystem** (`kernel/src/input.rs`) — Unified event queue for keyboard + mouse
3. **Cursor Renderer** (in `kernel/src/drivers/fbcon.rs` or new `kernel/src/drivers/cursor.rs`) — 16x16 arrow bitmap with save/restore
4. **GUI Event Loop** (refactored `gui_task` in `kernel/src/main.rs`) — Event-driven compositor
5. **Hit-Test System** (in `gui_task` or new `kernel/src/wm.rs`) — Determine which UI element is at (x, y)

## Component Details

### 1. PS/2 Mouse Driver

- IRQ 12 on PIC2 (interrupt 0x2C)
- Protocol: 3-byte packets `[buttons, dx, dy]`
- Enable sequence via port 0x64:
  1. 0xA8 — Enable mouse port
  2. 0x20 — Read command byte
  3. 0x60 — Write command byte with mouse enable bit
  4. Unmask IRQ 12 in PIC2 (port 0xA1, clear bit 4)
  5. Send 0xF4 to port 0x60 — Enable mouse data reporting
- State machine in IRQ handler: collect 3 bytes per packet
- Decode: `buttons` byte has bits 0=left, 1=right, 2=middle; `dx`/`dy` are signed deltas with overflow bits
- Stores `MouseState { x: u16, y: u16, left: bool, right: bool, middle: bool }` as atomic/locked struct
- aarch64 stub: returns no-op mouse state (QEMU virt machine doesn't have PS/2 mouse)

### 2. Input Subsystem (`kernel/src/input.rs`)

```rust
pub enum InputEvent {
    MouseMove { x: u16, y: u16, buttons: u8 },
    MouseDown { button: u8, x: u16, y: u16 },
    MouseUp { button: u8, x: u16, y: u16 },
    KeyPress { ascii: u8 },
}
```

- Ring buffer of 256 `InputEvent` entries
- Keyboard IRQ pushes `KeyPress` events (from existing `ps2kbd.rs` `handle_scancode` integration)
- Mouse IRQ pushes `MouseMove`, `MouseDown` (button press), `MouseUp` (button release)
- `pub fn init()` — sets up keyboard handler callback
- `pub fn push(event: InputEvent)` — called by IRQ handlers
- `pub fn poll() -> Option<InputEvent>` — non-blocking dequeue, called by gui_task each frame
- Thread-safe via `spin::Mutex`

### 3. Cursor Renderer

- 16x16 bitmap arrow cursor (hardcoded pixel data)
- `draw_cursor(x, y)` — saves pixels underneath, then draws cursor on top
- `undraw_cursor(x, y)` — restores saved pixels
- Cursor is drawn in the gui_task loop, not in IRQ handlers
- Position is clamped to framebuffer bounds
- Cursor color: white with black outline for visibility on any background

### 4. GUI Event Loop

Refactored `gui_task`:

```
1. Draw initial desktop
2. Draw cursor at center
3. Loop:
   a. Undraw cursor at old position (restore pixels)
   b. Process all pending input events from input::poll()
   c. Update cursor position, redraw cursor
   d. Handle UI events:
      - Dock hover: highlight icon under cursor
      - Dock click: bring shell window to front / focus
      - Title bar drag: move window with mouse
      - Traffic light click: close/minimize/maximize
      - KeyPress: forward to shell task
   e. scheduler::yield_cpu()
```

### 5. Hit-Test System

Simple function that checks UI element rectangles:

```rust
enum HitTarget {
    None,
    DockIcon(usize),       // Which dock icon index
    TrafficLight(TrafficLight), // Close/Minimize/Maximize
    TitleBar,              // For window dragging
    WindowBody,            // Inside the welcome window
}
```

Hit-test order (front to back):
1. Traffic light buttons (3 small circles at window top-left)
2. Title bar (rectangle below traffic lights)
3. Dock icons (row of squares at bottom)
4. Window body (the welcome window content area)

## Data Flow

```
PS/2 IRQ handlers (irq1, irq12)
  │
  ▼
input.rs event queue (ring buffer)
  │
  ▼
gui_task: while let Some(evt) = input::poll() { ... }
  │
  ├── MouseMove  ─► update cursor position ─► undraw/ redraw cursor ─► check hover
  ├── MouseDown  ─► hit_test(cursor_x, cursor_y) ─► dispatch action
  ├── MouseUp    ─► end drag if active
  └── KeyPress   ─► forward to shell task
```

## Constraints

- Cooperative scheduler only — no preemption. Input events are only processed when gui_task runs
- x86_64 only for mouse driver (aarch64 QEMU virt has no PS/2 mouse; will need virtio or device tree support later)
- Keyboard already works on both arches (UART on aarch64, PS/2 on x86_64)
- Single window for now (the welcome window); no window creation/destruction beyond traffic light buttons

## Testing

- QEMU x86_64: mouse should move cursor, click dock to activate shell, type in shell
- QEMU aarch64: keyboard input via UART (shell still works), no mouse expected
- No crash on input overflow or malformed PS/2 packets
