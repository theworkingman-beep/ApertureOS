//! VFS operations and per-process file descriptor table
//!
//! Manages file descriptors for processes, including stdin/stdout/stderr
//! virtual FDs and VFS-backed file descriptors.

use alloc::vec::Vec;
use core::ptr;
use spin::Mutex;

use super::pipe::pipe_create;
use super::FileHandle;
use crate::Stat;

/// Maximum number of file descriptors per process
pub const MAX_FDS: usize = 128;

/// File descriptor flags
pub const O_RDONLY: u32 = 0;
pub const O_WRONLY: u32 = 1;
pub const O_RDWR: u32 = 2;
pub const O_CREAT: u32 = 0x40;
pub const O_TRUNC: u32 = 0x200;
pub const O_APPEND: u32 = 0x400;

/// Seek whence constants
pub const SEEK_SET: u32 = 0;
pub const SEEK_CUR: u32 = 1;
pub const SEEK_END: u32 = 2;

/// File descriptor type
#[derive(Clone)]
pub enum FdKind {
    /// Standard input (fd 0)
    Stdin,
    /// Standard output (fd 1)
    Stdout,
    /// Standard error (fd 2)
    Stderr,
    /// VFS file handle
    Vfs(FileHandle),
    /// Pipe read end
    PipeRead(usize),
    /// Pipe write end
    PipeWrite(usize),
}

/// A file descriptor entry
#[derive(Clone)]
pub struct FileDescriptor {
    pub kind: FdKind,
    pub offset: u64,
    pub flags: u32,
}

/// Per-process file descriptor table
pub struct ProcessFdTable {
    fds: Vec<Option<FileDescriptor>>,
}

impl ProcessFdTable {
    pub fn new() -> Self {
        let mut fds = Vec::new();
        fds.resize(MAX_FDS, None);
        ProcessFdTable { fds }
    }

    /// Create a new fd table with stdin/stdout/stderr pre-allocated
    pub fn new_with_stdio() -> Self {
        let mut table = Self::new();
        table.fds[0] = Some(FileDescriptor {
            kind: FdKind::Stdin,
            offset: 0,
            flags: O_RDONLY,
        });
        table.fds[1] = Some(FileDescriptor {
            kind: FdKind::Stdout,
            offset: 0,
            flags: O_WRONLY,
        });
        table.fds[2] = Some(FileDescriptor {
            kind: FdKind::Stderr,
            offset: 0,
            flags: O_WRONLY,
        });
        table
    }

    /// Allocate a file descriptor slot, returning the fd number
    fn alloc_fd(&mut self) -> Option<usize> {
        // Skip 0, 1, 2 (reserved for stdio)
        for i in 3..MAX_FDS {
            if self.fds[i].is_none() {
                return Some(i);
            }
        }
        None
    }

    /// Get a reference to a file descriptor
    pub fn get(&self, fd: usize) -> Option<&FileDescriptor> {
        if fd < MAX_FDS {
            self.fds[fd].as_ref()
        } else {
            None
        }
    }

    /// Get a mutable reference to a file descriptor
    pub fn get_mut(&mut self, fd: usize) -> Option<&mut FileDescriptor> {
        if fd < MAX_FDS {
            self.fds[fd].as_mut()
        } else {
            None
        }
    }

    /// Insert a file descriptor at the next available slot
    pub fn insert(&mut self, desc: FileDescriptor) -> Option<usize> {
        let fd = self.alloc_fd()?;
        self.fds[fd] = Some(desc);
        Some(fd)
    }

    /// Insert a file descriptor at a specific slot (used for dup)
    pub fn insert_at(&mut self, fd: usize, desc: FileDescriptor) -> bool {
        if fd < MAX_FDS {
            self.fds[fd] = Some(desc);
            true
        } else {
            false
        }
    }

    /// Remove a file descriptor
    pub fn remove(&mut self, fd: usize) -> Option<FileDescriptor> {
        if fd < MAX_FDS {
            self.fds[fd].take()
        } else {
            None
        }
    }
}

/// Global registry of per-process FD tables, indexed by PID
static FD_TABLES: Mutex<Vec<(usize, ProcessFdTable)>> = Mutex::new(Vec::new());

/// Initialize the VFS ops subsystem
pub fn init() {
    log::info!("vfs_ops: initialized");
}

/// Create an FD table for a new process (called from scheduler when spawning)
pub fn create_fd_table(pid: usize) {
    let mut tables = FD_TABLES.lock();
    tables.push((pid, ProcessFdTable::new_with_stdio()));
}

/// Remove FD table when process exits
pub fn destroy_fd_table(pid: usize) {
    let mut tables = FD_TABLES.lock();
    if let Some(idx) = tables.iter().position(|(p, _)| *p == pid) {
        tables.remove(idx);
    }
}

/// Get FD table for a process (mutable)
fn get_fd_table_mut(pid: usize) -> Option<*mut ProcessFdTable> {
    // We need to be careful here. Since FD_TABLES is a Mutex, we can't
    // return a reference and also have the MutexGuard live.
    // Instead, we'll do operations under the lock.
    // This function is not used directly; operations are done inline with the lock.
    None // placeholder, we'll use inline operations instead
}

/// Open a file via VFS and assign an fd to the calling process
pub fn sys_open(pid: usize, path_ptr: usize, path_len: usize, flags: u32, _mode: u32) -> isize {
    // Read path from user memory
    if path_ptr == 0 || path_len == 0 || path_len > 4096 {
        return -1;
    }
    let path_slice = unsafe { core::slice::from_raw_parts(path_ptr as *const u8, path_len) };
    // Find null terminator or use full length
    let path_str_end = path_slice.iter().position(|&b| b == 0).unwrap_or(path_len);
    let path_str = match core::str::from_utf8(&path_slice[..path_str_end]) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    // Try to open via VFS
    let handle = {
        let vfs = super::VFS.lock();
        if let Some(fs) = vfs.fs {
            match fs.open(path_str) {
                Ok(h) => h,
                Err(_) => return -1,
            }
        } else {
            // No filesystem registered
            if (flags & O_CREAT) != 0 {
                // O_CREAT on no FS is not supported
                return -1;
            }
            return -1;
        }
    };

    // Get or create FD table for this PID
    let mut tables = FD_TABLES.lock();
    let entry = tables.iter_mut().find(|(p, _)| *p == pid);
    let fd = match entry {
        Some((_, table)) => {
            let desc = FileDescriptor {
                kind: FdKind::Vfs(handle),
                offset: 0,
                flags,
            };
            match table.insert(desc) {
                Some(fd) => fd,
                None => return -1,
            }
        }
        None => {
            // No FD table for this PID, create one
            drop(tables);
            create_fd_table(pid);
            let mut tables = FD_TABLES.lock();
            let entry = tables.iter_mut().find(|(p, _)| *p == pid);
            if let Some((_, table)) = entry {
                let desc = FileDescriptor {
                    kind: FdKind::Vfs(handle),
                    offset: 0,
                    flags,
                };
                match table.insert(desc) {
                    Some(fd) => fd,
                    None => return -1,
                }
            } else {
                return -1;
            }
        }
    };

    fd as isize
}

/// Close a file descriptor
pub fn sys_close(pid: usize, fd: usize) -> isize {
    let mut tables = FD_TABLES.lock();
    if let Some((_, table)) = tables.iter_mut().find(|(p, _)| *p == pid) {
        // Don't close stdin/stdout/stderr
        if fd < 3 {
            return 0;
        }
        match table.remove(fd) {
            Some(_) => 0,
            None => -1,
        }
    } else {
        -1
    }
}

/// Read from a file descriptor
pub fn sys_read(pid: usize, fd: usize, buf_ptr: usize, count: usize) -> isize {
    if buf_ptr == 0 || count == 0 {
        return 0;
    }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, count) };

    let mut tables = FD_TABLES.lock();
    let entry = tables.iter_mut().find(|(p, _)| *p == pid);
    match entry {
        Some((_, table)) => {
            match table.get_mut(fd) {
                Some(desc) => {
                    match &desc.kind {
                        FdKind::Stdin => {
                            // Read from input subsystem
                            let mut bytes_read = 0;
                            for i in 0..count {
                                if let Some(key) = crate::input::try_recv_key() {
                                    buf[i] = key as u8;
                                    bytes_read += 1;
                                } else {
                                    break;
                                }
                            }
                            bytes_read as isize
                        }
                        FdKind::Stdout | FdKind::Stderr => {
                            // Can't read from stdout/stderr
                            -1
                        }
                        FdKind::Vfs(handle) => {
                            // Read from VFS file
                            let offset = desc.offset as u32;
                            let vfs = super::VFS.lock();
                            if let Some(fs) = vfs.fs {
                                match fs.read(handle, offset, buf) {
                                    Ok(n) => {
                                        desc.offset += n as u64;
                                        n as isize
                                    }
                                    Err(_) => -1,
                                }
                            } else {
                                -1
                            }
                        }
                        FdKind::PipeRead(pipe_id) => {
                            let pid = *pipe_id;
                            drop(tables);
                            let mut pipes = crate::fs::pipe::PIPES.lock();
                            if pid < pipes.len() {
                                let n = pipes[pid].read(buf);
                                n as isize
                            } else {
                                -1
                            }
                        }
                        FdKind::PipeWrite(_) => {
                            // Can't read from pipe write end
                            -1
                        }
                    }
                }
                None => -1,
            }
        }
        None if fd == 0 => {
            // stdin without FD table
            let mut bytes_read = 0;
            for i in 0..count {
                if let Some(key) = crate::input::try_recv_key() {
                    buf[i] = key as u8;
                    bytes_read += 1;
                } else {
                    break;
                }
            }
            bytes_read as isize
        }
        None => -1,
    }
}

/// Write to a file descriptor
pub fn sys_write(pid: usize, fd: usize, buf_ptr: usize, count: usize) -> isize {
    if buf_ptr == 0 || count == 0 {
        return 0;
    }
    let data = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, count) };

    // Fast path for stdout/stderr
    if fd == 1 || fd == 2 {
        let mut written = 0;
        for &byte in data {
            if byte == 0 { break; }
            crate::drivers::uart::putc(byte);
            written += 1;
        }
        return written as isize;
    }

    let mut tables = FD_TABLES.lock();
    let entry = tables.iter_mut().find(|(p, _)| *p == pid);
    match entry {
        Some((_, table)) => {
            match table.get_mut(fd) {
                Some(desc) => {
                    match &desc.kind {
                        FdKind::Stdin => {
                            // Can't write to stdin
                            -1
                        }
                        FdKind::Stdout | FdKind::Stderr => {
                            for &byte in data {
                                if byte == 0 { break; }
                                crate::drivers::uart::putc(byte);
                            }
                            data.len() as isize
                        }
                        FdKind::Vfs(handle) => {
                            let handle = handle.clone();
                            // VFS write not fully supported yet (read-only FS)
                            // For now, return -1
                            drop(tables);
                            log::warn!("vfs_ops: write to VFS file not yet supported: {}", handle.path);
                            -1
                        }
                        FdKind::PipeRead(_) => {
                            // Can't write to pipe read end
                            -1
                        }
                        FdKind::PipeWrite(pipe_id) => {
                            let pid = *pipe_id;
                            drop(tables);
                            let mut pipes = crate::fs::pipe::PIPES.lock();
                            if pid < pipes.len() {
                                let n = pipes[pid].write(data);
                                n as isize
                            } else {
                                -1
                            }
                        }
                    }
                }
                None => -1,
            }
        }
        None => -1,
    }
}

/// Seek in a file descriptor
pub fn sys_seek(pid: usize, fd: usize, offset: i64, whence: u32) -> isize {
    let mut tables = FD_TABLES.lock();
    let entry = tables.iter_mut().find(|(p, _)| *p == pid);
    match entry {
        Some((_, table)) => {
            match table.get_mut(fd) {
                Some(desc) => {
                    match &desc.kind {
                        FdKind::Vfs(handle) => {
                            let new_offset = match whence {
                                SEEK_SET => offset as u64,
                                SEEK_CUR => {
                                    if offset >= 0 {
                                        desc.offset + offset as u64
                                    } else {
                                        desc.offset.saturating_sub((-offset) as u64)
                                    }
                                }
                                SEEK_END => handle.size as u64 + offset as u64,
                                _ => return -1,
                            };
                            desc.offset = new_offset;
                            new_offset as isize
                        }
                        _ => -1, // Can't seek on stdio or pipes
                    }
                }
                None => -1,
            }
        }
        None => -1,
    }
}

/// Get file status
pub fn sys_fstat(pid: usize, fd: usize, stat_ptr: usize) -> isize {
    if stat_ptr == 0 {
        return -1;
    }

    let mut tables = FD_TABLES.lock();
    let entry = tables.iter_mut().find(|(p, _)| *p == pid);
    match entry {
        Some((_, table)) => {
            match table.get(fd) {
                Some(desc) => {
                    let mut stat = Stat::default();
                    match &desc.kind {
                        FdKind::Stdin => {
                            stat.st_mode = 0o20000; // character device
                            stat.st_ino = 0;
                        }
                        FdKind::Stdout | FdKind::Stderr => {
                            stat.st_mode = 0o20000;
                            stat.st_ino = fd as u64;
                        }
                        FdKind::Vfs(handle) => {
                            stat.st_ino = handle.ino as u64;
                            stat.st_size = handle.size as u64;
                            stat.st_mode = 0o100000; // regular file
                            stat.st_blocks = (handle.size as u64 + 511) / 512;
                        }
                        FdKind::PipeRead(_) | FdKind::PipeWrite(_) => {
                            stat.st_mode = 0o10000; // fifo/pipe
                        }
                    }
                    unsafe {
                        ptr::write(stat_ptr as *mut Stat, stat);
                    }
                    0
                }
                None => -1,
            }
        }
        None => -1,
    }
}

/// Create a directory
pub fn sys_mkdir(_pid: usize, _path_ptr: usize, _path_len: usize, _mode: u32) -> isize {
    // Directory creation not yet supported on VFS
    log::warn!("vfs_ops: mkdir not yet implemented");
    -1
}

/// Unlink (delete) a file
pub fn sys_unlink(_pid: usize, _path_ptr: usize, _path_len: usize) -> isize {
    // File deletion not yet supported on VFS
    log::warn!("vfs_ops: unlink not yet implemented");
    -1
}

/// Duplicate a file descriptor
pub fn sys_dup(pid: usize, fd: usize) -> isize {
    let mut tables = FD_TABLES.lock();
    let entry = tables.iter_mut().find(|(p, _)| *p == pid);
    match entry {
        Some((_, table)) => {
            match table.get(fd) {
                Some(desc) => {
                    let new_desc = desc.clone();
                    match table.insert(new_desc) {
                        Some(new_fd) => new_fd as isize,
                        None => -1,
                    }
                }
                None => -1,
            }
        }
        None => -1,
    }
}

/// Create a pipe, returns (read_fd, write_fd) on success
pub fn sys_pipe(pid: usize, pipefd_ptr: usize) -> isize {
    if pipefd_ptr == 0 {
        return -1;
    }

    // Create pipe state
    let pipe_id = pipe_create();

    let mut tables = FD_TABLES.lock();
    let entry = tables.iter_mut().find(|(p, _)| *p == pid);
    match entry {
        Some((_, table)) => {
            let read_fd = match table.insert(FileDescriptor {
                kind: FdKind::PipeRead(pipe_id),
                offset: 0,
                flags: O_RDONLY,
            }) {
                Some(fd) => fd,
                None => return -1,
            };

            let write_fd = match table.insert(FileDescriptor {
                kind: FdKind::PipeWrite(pipe_id),
                offset: 0,
                flags: O_WRONLY,
            }) {
                Some(fd) => fd,
                None => {
                    table.remove(read_fd);
                    return -1;
                }
            };

            // Write [read_fd, write_fd] to user memory
            unsafe {
                ptr::write(pipefd_ptr as *mut i32, read_fd as i32);
                ptr::write((pipefd_ptr + 4) as *mut i32, write_fd as i32);
            }
            0
        }
        None => -1,
    }
}