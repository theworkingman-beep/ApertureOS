//! Simple IPC mailbox
use alloc::collections::vec_deque::VecDeque;
use spin::Mutex;

pub const IPC_PAYLOAD_SIZE: usize = 64;

#[derive(Debug, Clone, Copy)]
pub struct IpcMessage {
    pub sender: usize,
    pub msg_type: u8,
    pub payload: [u8; IPC_PAYLOAD_SIZE],
}

impl IpcMessage {
    pub fn new(sender: usize, msg_type: u8) -> Self {
        Self {
            sender,
            msg_type,
            payload: [0; IPC_PAYLOAD_SIZE],
        }
    }
}

static MAILBOXES: Mutex<VecDeque<(usize, IpcMessage)>> = Mutex::new(VecDeque::new());

pub fn init() {
    log::info!("ipc: initialized");
}

pub fn send(target: usize, msg: IpcMessage) {
    MAILBOXES.lock().push_back((target, msg));
}

pub fn recv(who: usize) -> Option<IpcMessage> {
    let mut q = MAILBOXES.lock();
    for i in 0..q.len() {
        if q[i].0 == who {
            return Some(q.remove(i).unwrap().1);
        }
    }
    None
}
