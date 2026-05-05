//! Networking stack (TCP/IP)

pub mod ethernet;
pub mod arp;
pub mod ipv4;
pub mod udp;
pub mod tcp;

pub fn init() {
    log::info!("net: initializing networking stack");
    // Call driver init here
    // virtio_net::init();
    // TODO: Initialize other protocols
}

/// Send a raw Ethernet frame
pub fn send_frame(_dst_mac: [u8; 6], _ethertype: u16, _payload: &[u8]) -> bool {
    // TODO: Implement frame sending
    false
}

/// Receive a raw Ethernet frame
pub fn recv_frame(_buf: &mut [u8]) -> usize {
    // TODO: Implement frame receiving
    0
}
