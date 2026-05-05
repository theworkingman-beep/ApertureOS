//! ARP (Address Resolution Protocol)

#![allow(dead_code)]

use core::ptr;

#[repr(C, packed)]
pub struct ArpHeader {
    pub hw_type: u16,
    pub proto_type: u16,
    pub hw_size: u8,
    pub proto_size: u8,
    pub opcode: u16,
    pub sender_mac: [u8; 6],
    pub sender_ip: [u8; 4],
    pub target_mac: [u8; 6],
    pub target_ip: [u8; 4],
}

pub const ARP_OPCODE_REQUEST: u16 = 1;
pub const ARP_OPCODE_REPLY: u16 = 2;

/// Handle an ARP packet
pub fn handle_arp(data: &[u8]) -> Option<()> {
    if data.len() < core::mem::size_of::<ArpHeader>() {
        return None;
    }
    let _hdr = unsafe { ptr::read_unaligned(data.as_ptr() as *const ArpHeader) };
    // TODO: Process ARP request/reply
    log::info!("arp: received packet (stub)");
    Some(())
}
