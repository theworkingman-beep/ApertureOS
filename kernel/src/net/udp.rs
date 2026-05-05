//! UDP protocol support

#![allow(dead_code)]

use core::ptr;

#[repr(C, packed)]
pub struct UdpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub len: u16,
    pub checksum: u16,
}

/// Parse UDP packet
pub fn parse(data: &[u8]) -> Option<(UdpHeader, &[u8])> {
    if data.len() < core::mem::size_of::<UdpHeader>() {
        return None;
    }
    let hdr = unsafe { ptr::read_unaligned(data.as_ptr() as *const UdpHeader) };
    let payload = &data[core::mem::size_of::<UdpHeader>()..];
    Some((hdr, payload))
}
