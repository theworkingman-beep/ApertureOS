//! TCP protocol support (minimal stub)

#![allow(dead_code)]

use core::ptr;

#[repr(C, packed)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset_flags: u16,
    pub window: u16,
    pub checksum: u16,
    pub urgent: u16,
}

/// Parse TCP segment
pub fn parse(data: &[u8]) -> Option<(TcpHeader, &[u8])> {
    if data.len() < core::mem::size_of::<TcpHeader>() {
        return None;
    }
    let hdr = unsafe { ptr::read_unaligned(data.as_ptr() as *const TcpHeader) };
    let hdr_len = ((hdr.data_offset_flags >> 12) & 0x0F) as usize * 4;
    if data.len() < hdr_len {
        return None;
    }
    let payload = &data[hdr_len..];
    Some((hdr, payload))
}
