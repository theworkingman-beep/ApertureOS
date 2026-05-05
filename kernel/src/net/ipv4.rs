//! IPv4 protocol support

#![allow(dead_code)]

use core::ptr;

#[repr(C, packed)]
pub struct IPv4Header {
    pub version_ihl: u8,
    pub dscp_ecn: u8,
    pub total_len: u16,
    pub identification: u16,
    pub flags_fragment: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
}

pub const PROTOCOL_ICMP: u8 = 1;
pub const PROTOCOL_TCP: u8 = 6;
pub const PROTOCOL_UDP: u8 = 17;

/// Parse IPv4 packet
pub fn parse(data: &[u8]) -> Option<(IPv4Header, &[u8])> {
    if data.len() < core::mem::size_of::<IPv4Header>() {
        return None;
    }
    let hdr = unsafe { ptr::read_unaligned(data.as_ptr() as *const IPv4Header) };
    let hdr_len = (hdr.version_ihl & 0x0F) as usize * 4;
    if data.len() < hdr_len {
        return None;
    }
    let payload = &data[hdr_len..];
    Some((hdr, payload))
}
