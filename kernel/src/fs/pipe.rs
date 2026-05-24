//! Pipe implementation for VibeOS
//!
//! Provides a 4KB ring buffer pipe for inter-process communication.

use spin::Mutex;
use alloc::collections::VecDeque;

/// Size of the pipe ring buffer
pub const PIPE_BUF_SIZE: usize = 4096;

/// Pipe state with a 4KB ring buffer
pub struct PipeState {
    /// Ring buffer for pipe data
    buffer: VecDeque<u8>,
    /// Whether the write end has been closed
    write_closed: bool,
}

impl PipeState {
    pub fn new() -> Self {
        PipeState {
            buffer: VecDeque::with_capacity(PIPE_BUF_SIZE),
            write_closed: false,
        }
    }

    /// Read up to `buf.len()` bytes from the pipe.
    /// Returns the number of bytes actually read.
    /// If the pipe is empty and write end is closed, returns 0 (EOF).
    /// If the pipe is empty and write end is open, returns 0 (caller should yield and retry).
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let mut i = 0;
        while i < buf.len() {
            if let Some(byte) = self.buffer.pop_front() {
                buf[i] = byte;
                i += 1;
            } else {
                break;
            }
        }
        i
    }

    /// Write bytes to the pipe. Returns the number of bytes written.
    /// Returns 0 if the pipe buffer is full.
    pub fn write(&mut self, data: &[u8]) -> usize {
        let mut written = 0;
        for &byte in data {
            if self.buffer.len() >= PIPE_BUF_SIZE {
                break;
            }
            self.buffer.push_back(byte);
            written += 1;
        }
        written
    }

    /// Close the write end of the pipe
    pub fn close_write(&mut self) {
        self.write_closed = true;
    }

    /// Check if the write end is closed and buffer is empty (EOF condition)
    pub fn is_eof(&self) -> bool {
        self.write_closed && self.buffer.is_empty()
    }

    /// Check if the pipe buffer is full
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= PIPE_BUF_SIZE
    }

    /// Check if the pipe buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

/// Global pipe registry
pub static PIPES: Mutex<VecDeque<PipeState>> = Mutex::new(VecDeque::new());

/// Create a new pipe and return its ID
pub fn pipe_create() -> usize {
    let mut pipes = PIPES.lock();
    let id = pipes.len();
    pipes.push_back(PipeState::new());
    id
}

/// Get a mutable reference to a pipe by ID
/// Returns None if the pipe doesn't exist
pub fn pipe_get(id: usize) -> Option<PipeState> {
    let pipes = PIPES.lock();
    if id < pipes.len() {
        Some(pipes[id].clone())
    } else {
        None
    }
}

impl Clone for PipeState {
    fn clone(&self) -> Self {
        PipeState {
            buffer: self.buffer.clone(),
            write_closed: self.write_closed,
        }
    }
}