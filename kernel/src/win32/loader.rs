//! PE/COFF loader for Windows executables.
//!
//! Reuses the architecture-independent `pe-parser` crate and adds process
//! address-space loading support.

pub use pe_parser::{parse_pe, parse_section_header, MachineType, PeImage, SectionHeader};

use super::process::Process;

/// Load a parsed PE image into a process address space.
pub fn load_into_process(image: &PeImage, process: &mut Process, data: &[u8]) -> bool {
    let total_pages = (image.image_size as usize + 4095) / 4096;
    let Some(base) = allocate_contiguous(total_pages) else {
        return false;
    };

    process.image_base = base;
    process.image_size = image.image_size;

    // Zero the allocated region before copying sections.
    unsafe {
        core::ptr::write_bytes(base as *mut u8, 0, total_pages * 4096);
    }

    // Copy each section from raw file offset to its virtual address.
    for i in 0..image.num_sections as usize {
        let offset = image.section_table_offset + i * 40;
        let Some(section) = parse_section_header(data, offset) else {
            return false;
        };
        map_section(process, &section, data, base);
    }

    // Record the entry point relative to the new base.
    let entry_rva = image.entry_point.saturating_sub(image.image_base);
    process.teb_base = base + entry_rva;

    true
}

fn map_section(process: &Process, section: &SectionHeader, data: &[u8], base: u64) {
    let dest = base + section.virtual_address as u64;
    let raw_size = section.raw_size as usize;
    let virtual_size = section.virtual_size as usize;

    let copy_size = raw_size.min(virtual_size);
    let src_offset = section.raw_offset as usize;

    if src_offset + copy_size <= data.len() {
        unsafe {
            core::ptr::copy_nonoverlapping(
                data.as_ptr().add(src_offset),
                dest as *mut u8,
                copy_size,
            );
        }
    }

    // Zero the remainder (BSS-style uninitialized data).
    if copy_size < virtual_size {
        unsafe {
            core::ptr::write_bytes((dest + copy_size as u64) as *mut u8, 0, virtual_size - copy_size);
        }
    }

    let _ = process; // Process metadata will be used for per-section permissions later.
}

fn allocate_contiguous(pages: usize) -> Option<u64> {
    if pages == 0 {
        return None;
    }
    // Allocate individual frames and verify they are contiguous.
    let first = crate::mm::frame_allocator::allocate()?;
    let mut last = first;
    for _ in 1..pages {
        let frame = crate::mm::frame_allocator::allocate()?;
        if frame != last + 4096 {
            // Not contiguous; simplistic fallback: fail.
            return None;
        }
        last = frame;
    }
    Some(first)
}

/// Determine if a guest PE architecture can run natively on the host.
pub fn requires_translation(guest: MachineType) -> bool {
    let host = host_machine();
    guest != host
}

fn host_machine() -> MachineType {
    #[cfg(target_arch = "x86_64")]
    {
        MachineType::Amd64
    }
    #[cfg(target_arch = "aarch64")]
    {
        MachineType::Arm64
    }
    #[cfg(target_arch = "x86")]
    {
        MachineType::I386
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86")))]
    {
        MachineType::Amd64
    }
}
