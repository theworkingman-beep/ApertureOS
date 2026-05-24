#![cfg_attr(not(feature = "std"), no_std)]

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum MemoryRegionKind {
    Usable,
    Reserved,
    Bootloader,
    Kernel,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub base: u64,
    pub length: u64,
    pub kind: MemoryRegionKind,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    pub addr: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u8,
}

#[repr(C)]
pub struct BootInfo {
    pub memory_map_ptr: *const MemoryRegion,
    pub memory_map_len: usize,
    pub framebuffer: *const FramebufferInfo,
    pub rsdp: u64,
    pub device_tree: u64,
}

/// POSIX-compatible stat structure for fstat syscall
#[repr(C)]
#[derive(Debug, Clone, Copy)]
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

impl Default for Stat {
    fn default() -> Self {
        Stat {
            st_dev: 0,
            st_ino: 0,
            st_mode: 0,
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            st_size: 0,
            st_blksize: 4096,
            st_blocks: 0,
            st_atime: 0,
            st_mtime: 0,
            st_ctime: 0,
        }
    }
}
