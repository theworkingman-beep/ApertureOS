use alloc::vec::Vec;
use crate::scheduler::Task;

const ELFMAG: [u8; 4] = *b"\x7fELF";
const ELFCLASS64: u8 = 2;
const ET_EXEC: u16 = 2;
const PT_LOAD: u32 = 1;

#[repr(C)]
struct Elf64Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
struct Elf64Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

/// Load an ELF64 executable into a user task's address space
/// Returns (entry_point, stack_top) on success
pub fn load_elf_for_task(elf_data: &[u8], task: &mut Task) -> Option<(u64, u64)> {
    if elf_data.len() < core::mem::size_of::<Elf64Ehdr>() { return None; }

    let hdr = unsafe { &*(elf_data.as_ptr() as *const Elf64Ehdr) };

    // Verify ELF magic, 64-bit, executable
    if &hdr.e_ident[..4] != &ELFMAG { return None; }
    if hdr.e_ident[4] != ELFCLASS64 { return None; }
    if hdr.e_type != ET_EXEC { return None; }

    // Verify machine architecture matches current arch
    #[cfg(target_arch = "x86_64")]
    if hdr.e_machine != 62 { return None; } // EM_X86_64
    #[cfg(target_arch = "aarch64")]
    if hdr.e_machine != 183 { return None; } // EM_AARCH64

    let entry = hdr.e_entry;
    let ph_off = hdr.e_phoff as usize;
    let ph_size = core::mem::size_of::<Elf64Phdr>() as u64;

    // User stack top (in user address space)
    #[cfg(target_arch = "x86_64")]
    let stack_top: u64 = 0x7FFFFFFFF000; // Top of user space for x86_64
    #[cfg(target_arch = "aarch64")]
    let stack_top: u64 = 0x0000FFFFFFFFF000; // Top of user space for aarch64

    // Map user stack pages (4 pages = 16KB stack)
    if let Some(ref mut pt) = task.page_tables {
        for i in 0..4 {
            let stack_page_vaddr = stack_top - (i as u64 + 1) * 4096;
            unsafe {
                #[cfg(target_arch = "x86_64")]
                let _ = crate::arch::x86_64::map_user_page_for_task(pt, stack_page_vaddr, true);
                #[cfg(target_arch = "aarch64")]
                let _ = crate::arch::aarch64::map_user_page_for_task(pt, stack_page_vaddr, true);
            }
        }
    }

    // Process each program header
    for i in 0..hdr.e_phnum {
        let off = ph_off + (i as u64 * ph_size) as usize;
        if off + ph_size as usize > elf_data.len() { break; }

        let ph = unsafe { &*(elf_data.as_ptr().add(off) as *const Elf64Phdr) };

        if ph.p_type != PT_LOAD { continue; }

        let vaddr = ph.p_vaddr;
        let filesz = ph.p_filesz;
        let memsz = ph.p_memsz;
        let offset = ph.p_offset;

        // Map pages for this segment
        let page_start = vaddr & !0xFFF;
        let page_end = (vaddr + memsz + 4095) & !0xFFF;

        for page_vaddr in (page_start..page_end).step_by(4096) {
            if let Some(ref mut pt) = task.page_tables {
                // Allocate physical frame and map it
                let frame_opt = unsafe {
                    #[cfg(target_arch = "x86_64")]
                    let frame = crate::arch::x86_64::map_user_page_for_task(pt, page_vaddr, true);
                    #[cfg(target_arch = "aarch64")]
                    let frame = crate::arch::aarch64::map_user_page_for_task(pt, page_vaddr, true);
                    frame
                };
                if let Some(frame_phys) = frame_opt {
                    // Copy data to this page
                    let page_offset = page_vaddr - page_start;
                    let copy_start = offset + page_offset;
                    let copy_len = core::cmp::min(4096, filesz.saturating_sub(page_offset));

                    if copy_start < elf_data.len() as u64 && copy_len > 0 {
                        let src_start = core::cmp::min(copy_start as usize, elf_data.len());
                        let copy_amount = core::cmp::min(copy_len as usize, elf_data.len() - src_start);

                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                elf_data.as_ptr().add(src_start),
                                frame_phys as *mut u8,
                                copy_amount,
                            );
                        }
                    }
                }
            }
        }
    }

    log::info!("ELF loaded: entry={:#x}, stack_top={:#x}", entry, stack_top);
    Some((entry, stack_top))
}

/// Simple ELF loader that just parses and returns info (used for testing)
pub fn load_elf(data: &[u8]) -> Option<u64> {
    if data.len() < core::mem::size_of::<Elf64Ehdr>() { return None; }
    let hdr = unsafe { &*(data.as_ptr() as *const Elf64Ehdr) };
    if &hdr.e_ident[..4] != &ELFMAG || hdr.e_ident[4] != ELFCLASS64 || hdr.e_type != ET_EXEC {
        return None;
    }
    Some(hdr.e_entry)
}
