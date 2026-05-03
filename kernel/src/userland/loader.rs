use alloc::vec::Vec;

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

#[derive(Debug)]
pub struct ElfImage {
    pub entry: u64,
    pub segments: Vec<(u64, u64, usize)>, // (vaddr, size, ptr)
}

pub fn load_elf(data: &[u8]) -> Option<ElfImage> {
    if data.len() < core::mem::size_of::<Elf64Ehdr>() { return None; }
    let hdr = unsafe { &*(data.as_ptr() as *const Elf64Ehdr) };
    if &hdr.e_ident[..4] != &ELFMAG || hdr.e_ident[4] != ELFCLASS64 || hdr.e_type != ET_EXEC {
        return None;
    }
    let ph_off = hdr.e_phoff as usize;
    let ph_size = core::mem::size_of::<Elf64Phdr>();
    let mut segments = Vec::new();
    for i in 0..hdr.e_phnum {
        let off = ph_off + (i as usize) * ph_size;
        if off + ph_size > data.len() { break; }
        let ph = unsafe { &*(data.as_ptr().add(off) as *const Elf64Phdr) };
        if ph.p_type == PT_LOAD {
            let layout = core::alloc::Layout::from_size_align(ph.p_memsz as usize, 4096).ok()?;
            let ptr = unsafe { alloc::alloc::alloc(layout) };
            if ptr.is_null() { return None; }
            unsafe {
                core::ptr::copy_nonoverlapping(
                    data.as_ptr().add(ph.p_offset as usize),
                    ptr,
                    ph.p_filesz as usize,
                );
                core::ptr::write_bytes(ptr.add(ph.p_filesz as usize), 0, (ph.p_memsz - ph.p_filesz) as usize);
            }
            segments.push((ph.p_vaddr, ph.p_memsz, ptr as usize));
        }
    }
    Some(ElfImage { entry: hdr.e_entry, segments })
}
