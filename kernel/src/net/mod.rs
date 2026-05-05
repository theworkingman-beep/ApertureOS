//! Networking stack (TCP/IP)
//!
//! Implements a basic TCP/IP stack for VibeOS.
//! Uses virtio-net for Ethernet frame transmission.

#![allow(dead_code)]

use core::ptr;
use spin::Mutex;

pub mod ethernet;
pub mod arp;
pub mod ipv4;
pub mod udp;
pub mod tcp;
mod virtio_net;

/// Local MAC address
static LOCAL_MAC: Mutex<[u8; 6]> = Mutex::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);

/// Local IP address (QEMU user-mode networking)
static LOCAL_IP: Mutex<[u8; 4]> = Mutex::new([10, 0, 2, 15]);

/// Initialize the networking stack
pub fn init() {
    log::info!("net: initializing networking stack");

    if !virtio_net::init() {
        log::warn!("net: virtio-net initialization failed");
        return;
    }

    let mac = virtio_net::get_mac();
    *LOCAL_MAC.lock() = mac;

    log::info!("net: MAC = {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
}

/// Send a raw Ethernet frame
pub fn send_frame(dst_mac: [u8; 6], ethertype: u16, payload: &[u8]) -> bool {
    virtio_net::send_frame(dst_mac, ethertype, payload)
}

/// Receive a raw Ethernet frame
pub fn recv_frame(buf: &mut [u8]) -> usize {
    virtio_net::recv_frame(buf)
}

/// Poll for incoming packets
pub fn poll() {
    let mut buf = [0u8; 1526];
    let len = recv_frame(&mut buf);
    if len > 0 {
        process_frame(&buf[..len]);
    }
}

/// Process a received Ethernet frame
fn process_frame(frame: &[u8]) {
    match ethernet::parse(frame) {
        Some((hdr, payload)) => {
            match hdr.ethertype() {
                ethernet::ETHERTYPE_ARP => {
                    arp::handle_arp(payload);
                }
                ethernet::ETHERTYPE_IPV4 => {
                    handle_ipv4(payload);
                }
                _ => {}
            }
        }
        None => {}
    }
}

/// Handle IPv4 packet
fn handle_ipv4(data: &[u8]) {
    match ipv4::parse(data) {
        Some((hdr, _payload)) => {
            if hdr.dst_ip != *LOCAL_IP.lock() {
                return;
            }
            match hdr.protocol {
                ipv4::PROTOCOL_UDP => {
                    log::debug!("net: UDP packet");
                }
                ipv4::PROTOCOL_TCP => {
                    log::debug!("net: TCP packet");
                }
                _ => {}
            }
        }
        None => {}
    }
}

/// Get local IP
pub fn get_local_ip() -> [u8; 4] {
    *LOCAL_IP.lock()
}

/// Get local MAC
pub fn get_local_mac() -> [u8; 6] {
    *LOCAL_MAC.lock()
}
