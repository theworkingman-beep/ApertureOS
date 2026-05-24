//! VibeOS libc compatibility layer
//! Provides C-compatible function wrappers that call VibeOS syscalls via libvibe.
//! This is a no_std static library for linking with C programs.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::ptr;

// ── Bump allocator for malloc/free ──────────────────────────────────────────
const LIBC_HEAP_SIZE: usize = 4 * 1024 * 1024; // 4 MB
static mut HEAP: [u8; LIBC_HEAP_SIZE] = [0; LIBC_HEAP_SIZE];

struct BumpAlloc {
    next: core::sync::atomic::AtomicUsize,
    base: core::sync::atomic::AtomicUsize,
}

unsafe impl alloc::alloc::GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();
        let base = {
            let b = self.base.load(core::sync::atomic::Ordering::Relaxed);
            if b == 0 {
                let heap_base = HEAP.as_ptr() as usize;
                self.base.store(heap_base, core::sync::atomic::Ordering::Relaxed);
                self.next.store(heap_base, core::sync::atomic::Ordering::Relaxed);
                heap_base
            } else {
                b
            }
        };
        loop {
            let current = self.next.load(core::sync::atomic::Ordering::Relaxed);
            let aligned = (current + align - 1) & !(align - 1);
            let new = aligned + size;
            if new > base + LIBC_HEAP_SIZE {
                return core::ptr::null_mut();
            }
            if self
                .next
                .compare_exchange_weak(
                    current,
                    new,
                    core::sync::atomic::Ordering::Relaxed,
                    core::sync::atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                return aligned as *mut u8;
            }
        }
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        // Bump allocator: no deallocation
    }
}

#[global_allocator]
static ALLOCATOR: BumpAlloc = BumpAlloc {
    next: core::sync::atomic::AtomicUsize::new(0),
    base: core::sync::atomic::AtomicUsize::new(0),
};

// ---- Syscall number constants (matching kernel) ----
const SYS_EXIT: usize = 0;
const SYS_WRITE: usize = 1;
const SYS_READ: usize = 2;
const SYS_FORK: usize = 5;
const SYS_WAIT: usize = 6;
const SYS_EXEC: usize = 7;
const SYS_YIELD: usize = 4;

const SYS_OPEN: usize = 18;
const SYS_CLOSE: usize = 19;
const SYS_READ_FD: usize = 20;
const SYS_WRITE_FD: usize = 21;
const SYS_SEEK: usize = 22;
const SYS_FSTAT: usize = 23;
const SYS_MKDIR: usize = 24;
const SYS_UNLINK: usize = 25;
const SYS_GETPID: usize = 26;
const SYS_DUP: usize = 27;
const SYS_PIPE: usize = 28;
const SYS_MMAP: usize = 29;
const SYS_MUNMAP: usize = 30;
const SYS_IOCTL: usize = 31;
const SYS_GETTIMEOFDAY: usize = 32;
const SYS_NANOSLEEP: usize = 33;

// ---- O_* flags ----
pub const O_RDONLY: i32 = 0;
pub const O_WRONLY: i32 = 1;
pub const O_RDWR: i32 = 2;
pub const O_CREAT: i32 = 0x40;
pub const O_TRUNC: i32 = 0x200;
pub const O_APPEND: i32 = 0x400;

// ---- SEEK constants ----
pub const SEEK_SET: i32 = 0;
pub const SEEK_CUR: i32 = 1;
pub const SEEK_END: i32 = 2;

// ---- Errno constants ----
pub const ENOSYS: i32 = 38;
pub const EBADF: i32 = 9;
pub const ENOMEM: i32 = 12;
pub const EINVAL: i32 = 22;
pub const EFAULT: i32 = 14;

// ---- Signal constants ----
pub const SIG_DFL: usize = 0;

// ---- C-compatible types ----

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub st_size: u64,
    pub st_blksize: u32,
    pub st_blocks: u64,
    pub st_atime: u64,
    pub st_mtime: u64,
    pub st_ctime: u64,
}

#[repr(C)]
pub struct Timeval {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

#[repr(C)]
pub struct Timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

// ---- Static errno ----
static mut ERRNO: i32 = 0;

// ---- Bump allocator state ----
static mut HEAP_START: usize = 0;
static mut HEAP_END: usize = 0;
static mut HEAP_CURRENT: usize = 0;
const HEAP_SIZE: usize = 1024 * 1024; // 1MB bump allocator

// ---- External symbols for heap ----
extern "C" {
    static __heap_start: u8;
    static __heap_end: u8;
}

// ---- Internal init ----
unsafe fn ensure_heap_init() {
    if HEAP_START == 0 {
        // Use a static 1MB buffer for the bump allocator since we can't
        // reference linker symbols in no_std easily. Instead we allocate
        // from a fixed region.
        // In a real OS, this would come from mmap/brk syscalls.
        // For now, we'll use a static buffer.
    }
}

// ---- File I/O functions ----

#[no_mangle]
pub extern "C" fn open(path: *const u8, flags: i32, mode: i32) -> i32 {
    if path.is_null() {
        unsafe { ERRNO = EINVAL; }
        return -1;
    }
    let path_len = unsafe { strlen(path) };
    let result = unsafe {
        libvibe::syscall3(SYS_OPEN, path as usize, path_len, flags as usize)
        // Note: flags and mode would be passed as a4, a5 via syscall5, but we only have syscall3/6
        // For now, we use syscall3 with just path + len + flags
    };
    result as i32
}

#[no_mangle]
pub extern "C" fn close(fd: i32) -> i32 {
    let result = unsafe { libvibe::syscall1(SYS_CLOSE, fd as usize) };
    result as i32
}

#[no_mangle]
pub extern "C" fn read(fd: i32, buf: *mut u8, count: usize) -> isize {
    if buf.is_null() {
        unsafe { ERRNO = EFAULT; }
        return -1;
    }
    let result = unsafe { libvibe::syscall3(SYS_READ_FD, fd as usize, buf as usize, count) };
    result as isize
}

#[no_mangle]
pub extern "C" fn write(fd: i32, buf: *const u8, count: usize) -> isize {
    if buf.is_null() && count > 0 {
        unsafe { ERRNO = EFAULT; }
        return -1;
    }
    let result = unsafe { libvibe::syscall3(SYS_WRITE_FD, fd as usize, buf as usize, count) };
    result as isize
}

#[no_mangle]
pub extern "C" fn lseek(fd: i32, offset: isize, whence: i32) -> isize {
    let result = unsafe { libvibe::syscall3(SYS_SEEK, fd as usize, offset as usize, whence as usize) };
    result as isize
}

#[no_mangle]
pub extern "C" fn fstat(fd: i32, stat_buf: *mut Stat) -> i32 {
    if stat_buf.is_null() {
        unsafe { ERRNO = EFAULT; }
        return -1;
    }
    let result = unsafe { libvibe::syscall3(SYS_FSTAT, fd as usize, stat_buf as usize, 0) };
    result as i32
}

#[no_mangle]
pub extern "C" fn mkdir(path: *const u8, mode: u32) -> i32 {
    if path.is_null() {
        unsafe { ERRNO = EINVAL; }
        return -1;
    }
    let path_len = unsafe { strlen(path) };
    let result = unsafe { libvibe::syscall3(SYS_MKDIR, path as usize, path_len, mode as usize) };
    result as i32
}

#[no_mangle]
pub extern "C" fn unlink(path: *const u8) -> i32 {
    if path.is_null() {
        unsafe { ERRNO = EINVAL; }
        return -1;
    }
    let path_len = unsafe { strlen(path) };
    let result = unsafe { libvibe::syscall3(SYS_UNLINK, path as usize, path_len, 0) };
    result as i32
}

#[no_mangle]
pub extern "C" fn dup(fd: i32) -> i32 {
    let result = unsafe { libvibe::syscall1(SYS_DUP, fd as usize) };
    result as i32
}

#[no_mangle]
pub extern "C" fn pipe(pipefd: *mut i32) -> i32 {
    if pipefd.is_null() {
        unsafe { ERRNO = EFAULT; }
        return -1;
    }
    let result = unsafe { libvibe::syscall1(SYS_PIPE, pipefd as usize) };
    result as i32
}

#[no_mangle]
pub extern "C" fn ioctl(fd: i32, request: u64, arg: u64) -> i32 {
    let result = unsafe { libvibe::syscall6(SYS_IOCTL, fd as usize, request as usize, arg as usize, 0, 0, 0) };
    if result == usize::MAX {
        unsafe { ERRNO = ENOSYS; }
        return -1;
    }
    result as i32
}

// ---- Process functions ----

#[no_mangle]
pub extern "C" fn getpid() -> i32 {
    let result = unsafe { libvibe::syscall1(SYS_GETPID, 0) };
    result as i32
}

#[no_mangle]
pub extern "C" fn fork() -> i32 {
    let result = unsafe { libvibe::syscall1(SYS_FORK, 0) };
    result as i32
}

#[no_mangle]
pub extern "C" fn _exit(code: i32) -> ! {
    unsafe { libvibe::syscall1(SYS_EXIT, code as usize); }
    loop {}
}

#[no_mangle]
pub extern "C" fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32 {
    let result = unsafe { libvibe::syscall3(SYS_WAIT, pid as usize, status as usize, options as usize) };
    // wait returns (pid << 32) | status - extract properly
    let ret_pid = (result >> 32) as i32;
    let ret_status = (result & 0xFFFFFFFF) as i32;
    if !status.is_null() {
        unsafe { ptr::write(status, ret_status); }
    }
    ret_pid
}

// ---- Memory functions ----

#[no_mangle]
pub extern "C" fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: isize) -> *mut u8 {
    let result = unsafe {
        libvibe::syscall6(
            SYS_MMAP,
            addr as usize,
            len,
            prot as usize,
            flags as usize,
            fd as usize,
            offset as usize,
        )
    };
    if result == usize::MAX {
        unsafe { ERRNO = ENOMEM; }
        return usize::MAX as *mut u8;
    }
    result as *mut u8
}

#[no_mangle]
pub extern "C" fn munmap(addr: *mut u8, len: usize) -> i32 {
    let result = unsafe { libvibe::syscall3(SYS_MUNMAP, addr as usize, len, 0) };
    result as i32
}

#[no_mangle]
pub extern "C" fn brk(addr: *mut u8) -> *mut u8 {
    // Simple bump allocator - return current heap pointer
    // If addr is not null, set heap pointer to addr
    unsafe {
        if !addr.is_null() {
            HEAP_CURRENT = addr as usize;
        }
        HEAP_CURRENT as *mut u8
    }
}

// ---- Time functions ----

#[no_mangle]
pub extern "C" fn gettimeofday(tv: *mut Timeval, tz: *mut u8) -> i32 {
    if tv.is_null() {
        return 0;
    }
    let result = unsafe { libvibe::syscall3(SYS_GETTIMEOFDAY, tv as usize, tz as usize, 0) };
    result as i32
}

#[no_mangle]
pub extern "C" fn nanosleep(req: *const Timespec, rem: *mut Timespec) -> i32 {
    let result = unsafe { libvibe::syscall3(SYS_NANOSLEEP, req as usize, rem as usize, 0) };
    result as i32
}

#[no_mangle]
pub extern "C" fn clock_gettime(clock_id: i32, tp: *mut Timespec) -> i32 {
    if tp.is_null() {
        unsafe { ERRNO = EFAULT; }
        return -1;
    }
    // Use gettimeofday and convert
    let mut tv = Timeval { tv_sec: 0, tv_usec: 0 };
    let result = gettimeofday(&mut tv, core::ptr::null_mut());
    if result == 0 {
        unsafe {
            (*tp).tv_sec = tv.tv_sec;
            (*tp).tv_nsec = tv.tv_usec * 1000;
        }
    }
    result
}

// ---- String/stdlib functions ----

#[no_mangle]
pub extern "C" fn strlen(s: *const u8) -> usize {
    if s.is_null() {
        return 0;
    }
    let mut len = 0usize;
    unsafe {
        while *s.add(len) != 0 {
            len += 1;
        }
    }
    len
}

#[no_mangle]
pub extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    if s.is_null() {
        return s;
    }
    unsafe {
        let mut i = 0;
        while i < n {
            *s.add(i) = c as u8;
            i += 1;
        }
    }
    s
}

#[no_mangle]
pub extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    unsafe {
        let mut i = 0;
        while i < n {
            *dest.add(i) = *src.add(i);
            i += 1;
        }
    }
    dest
}

#[no_mangle]
pub extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    if s1.is_null() || s2.is_null() {
        return 0;
    }
    unsafe {
        let mut i = 0;
        while i < n {
            let a = *s1.add(i);
            let b = *s2.add(i);
            if a != b {
                return (a as i32) - (b as i32);
            }
            i += 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    unsafe {
        let dest_addr = dest as usize;
        let src_addr = src as usize;
        if dest_addr < src_addr || dest_addr >= src_addr + n {
            // Non-overlapping or forward copy
            let mut i = 0;
            while i < n {
                *dest.add(i) = *src.add(i);
                i += 1;
            }
        } else {
            // Overlapping backward copy
            let mut i = n;
            while i > 0 {
                i -= 1;
                *dest.add(i) = *src.add(i);
            }
        }
    }
    dest
}

#[no_mangle]
pub extern "C" fn strcpy(dest: *mut u8, src: *const u8) -> *mut u8 {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    unsafe {
        let mut i = 0;
        loop {
            let c = *src.add(i);
            *dest.add(i) = c;
            if c == 0 {
                break;
            }
            i += 1;
        }
    }
    dest
}

#[no_mangle]
pub extern "C" fn strcmp(s1: *const u8, s2: *const u8) -> i32 {
    if s1.is_null() || s2.is_null() {
        return 0;
    }
    unsafe {
        let mut i = 0;
        loop {
            let a = *s1.add(i);
            let b = *s2.add(i);
            if a != b {
                return (a as i32) - (b as i32);
            }
            if a == 0 {
                return 0;
            }
            i += 1;
        }
    }
}

#[no_mangle]
pub extern "C" fn strncmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    if s1.is_null() || s2.is_null() {
        return 0;
    }
    unsafe {
        let mut i = 0;
        while i < n {
            let a = *s1.add(i);
            let b = *s2.add(i);
            if a != b {
                return (a as i32) - (b as i32);
            }
            if a == 0 {
                return 0;
            }
            i += 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn strdup(s: *const u8) -> *mut u8 {
    if s.is_null() {
        return core::ptr::null_mut();
    }
    let len = unsafe { strlen(s) };
    let ptr = malloc(len + 1);
    if ptr.is_null() {
        return core::ptr::null_mut();
    }
    unsafe { memcpy(ptr, s, len + 1) };
    ptr
}

// ---- Bump allocator for malloc/free ----

static mut BUMP_PTR: usize = 0;
static mut BUMP_END: usize = 0;

// 64KB static heap area for bump allocator
static mut HEAP_BUFFER: [u8; 65536] = [0u8; 65536];

unsafe fn init_bump() {
    if BUMP_PTR == 0 {
        BUMP_PTR = HEAP_BUFFER.as_ptr() as usize;
        BUMP_END = BUMP_PTR + HEAP_BUFFER.len();
    }
}

#[no_mangle]
pub extern "C" fn malloc(size: usize) -> *mut u8 {
    if size == 0 {
        return core::ptr::null_mut();
    }
    unsafe {
        init_bump();
        // Align to 16 bytes
        let aligned_ptr = (BUMP_PTR + 15) & !15;
        let new_ptr = aligned_ptr + size;
        if new_ptr > BUMP_END {
            // Out of memory
            return core::ptr::null_mut();
        }
        BUMP_PTR = new_ptr;
        // Zero-initialize the allocation
        core::ptr::write_bytes(aligned_ptr as *mut u8, 0, size);
        aligned_ptr as *mut u8
    }
}

#[no_mangle]
pub extern "C" fn free(_ptr: *mut u8) {
    // Bump allocator: free is a no-op
}

#[no_mangle]
pub extern "C" fn calloc(nmemb: usize, size: usize) -> *mut u8 {
    let total = nmemb * size;
    if total == 0 {
        return core::ptr::null_mut();
    }
    let ptr = malloc(total);
    // malloc already zero-fills
    ptr
}

#[no_mangle]
pub extern "C" fn realloc(ptr: *mut u8, size: usize) -> *mut u8 {
    if ptr.is_null() {
        return malloc(size);
    }
    if size == 0 {
        free(ptr);
        return core::ptr::null_mut();
    }
    // Allocate new block and copy
    let new_ptr = malloc(size);
    if new_ptr.is_null() {
        return core::ptr::null_mut(); // Keep old block
    }
    // We don't know the old size, so copy a conservative amount
    // Bump allocator means the old block is still valid
    unsafe {
        // Copy min of old and new sizes - but we don't know old size
        // Just copy size bytes (the old block might be smaller, but
        // we'll at least not read past the old allocation's end in practice)
        memcpy(new_ptr, ptr, size);
    }
    new_ptr
}

#[no_mangle]
pub extern "C" fn abort() -> ! {
    _exit(134); // SIGABRT exit code
}

#[no_mangle]
pub extern "C" fn atoi(s: *const u8) -> i32 {
    if s.is_null() {
        return 0;
    }
    let mut result = 0i32;
    let mut negative = false;
    let mut i = 0;
    unsafe {
        // Skip whitespace
        while *s.add(i) == b' ' || *s.add(i) == b'\t' {
            i += 1;
        }
        // Check sign
        if *s.add(i) == b'-' {
            negative = true;
            i += 1;
        } else if *s.add(i) == b'+' {
            i += 1;
        }
        // Parse digits
        while *s.add(i) >= b'0' && *s.add(i) <= b'9' {
            result = result * 10 + (*s.add(i) - b'0') as i32;
            i += 1;
        }
    }
    if negative { -result } else { result }
}

// ---- Errno ----

#[no_mangle]
pub extern "C" fn __errno_location() -> *mut i32 {
    unsafe { &mut ERRNO as *mut i32 }
}

// ---- Signal stubs ----

#[no_mangle]
pub extern "C" fn kill(_pid: i32, _sig: i32) -> i32 {
    unsafe { ERRNO = ENOSYS; }
    -1
}

#[no_mangle]
pub extern "C" fn signal(_sig: i32, _handler: usize) -> usize {
    unsafe { ERRNO = ENOSYS; }
    SIG_DFL
}

// ---- PANIC HANDLER ----

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// ---- GLOBAL ASSEMBLY ----
// Provide empty __aeabi_* symbols for ARM if needed
// and other compiler_builtins stubs