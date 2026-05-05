//! Ethernet frame handling
//! 
//! Supports basic Ethernet II frame parsing and construction.

#![allow(dead_code)]

use core::ptr;

#[repr(C, packed)]
pub struct EthernetHeader {
    pub dst_mac: [u8; 6],
    pub src_mac: [u8; 6],
    pub ethertype: u16,
}

impl EthernetHeader {
    pub fn new(dst: [u8; 6], src: [u8; 6], ethertype: u16) -> Self {
        Self {
            dst_mac: dst,
            src_mac: src,
            ethertype: ethertype.to_be(),
        }
    }

    pub fn ethertype(&self) -> u16 {
        u16::from_be(self.ethertype)
    }
}

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;
pub const ETHERTYPE_IPV6: u16 = 0x86DD;

/// Parse an Ethernet frame from raw bytes
pub fn parse(data: &[u8]) -> Option<(EthernetHeader, &[u8])> {
    if data.len() < core::mem::size_of::<EthernetHeader>() {
        return None;
    }
    let hdr = unsafe { ptr::read_unaligned(data.as_ptr() as *const EthernetHeader) };
    let payload = &data[core::mem::size_of::<EthernetHeader>()..];
    Some((hdr, payload))
}
