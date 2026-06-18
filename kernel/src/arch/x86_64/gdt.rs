//! Global Descriptor Table and Task State Segment setup for x86_64.
//!
//! The TSS provides a ring-0 stack pointer used by the CPU when interrupts
//! arrive from user mode, and a separate IST entry for double-fault so that
//! stack exhaustion does not recurse. The GDT holds the code/data segments
//! required by SYSCALL/SYSRET and the TSS descriptor.

#![allow(static_mut_refs)]

use x86_64::instructions::segmentation::{Segment, CS, DS, ES, FS, GS, SS};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

const STACK_SIZE: usize = 32 * 1024; // 32 KiB kernel stacks

// Ring-0 interrupt stack and double-fault IST stack.
static mut RING0_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
static mut DOUBLE_FAULT_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

static TSS: TaskStateSegment = TaskStateSegment::new();
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

#[derive(Clone, Copy)]
pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub user_data: SegmentSelector,
    pub tss: SegmentSelector,
}

static mut SELECTORS: Option<Selectors> = None;

/// Return the selectors used by SYSCALL/SYSRET configuration.
pub fn selectors() -> Selectors {
    unsafe { SELECTORS.expect("GDT not initialized") }
}

/// Load the GDT and TSS.
///
/// # Safety
/// Must be called exactly once from valid x86_64 long mode.
pub unsafe fn init() {
    let ring0_top = core::ptr::addr_of!(RING0_STACK) as u64 + STACK_SIZE as u64;
    let df_top = core::ptr::addr_of!(DOUBLE_FAULT_STACK) as u64 + STACK_SIZE as u64;
    let tss_ptr = core::ptr::addr_of!(TSS) as *mut TaskStateSegment;
    unsafe {
        (*tss_ptr).privilege_stack_table[0] = x86_64::VirtAddr::new(ring0_top);
        (*tss_ptr).interrupt_stack_table[0] = x86_64::VirtAddr::new(df_top);
    }

    // Order matters for SYSCALL/SYSRET: the CPU computes SS = CS + 8, so
    // kernel code/data must be followed by user code/data.
    let kernel_code = GDT.append(Descriptor::kernel_code_segment());
    let kernel_data = GDT.append(Descriptor::kernel_data_segment());
    let user_code = GDT.append(Descriptor::user_code_segment());
    let user_data = GDT.append(Descriptor::user_data_segment());
    let tss_selector = GDT.append(Descriptor::tss_segment(&TSS));
    SELECTORS = Some(Selectors {
        kernel_code,
        kernel_data,
        user_code,
        user_data,
        tss: tss_selector,
    });

    GDT.load();

    CS::set_reg(kernel_code);
    SS::set_reg(kernel_data);
    DS::set_reg(SegmentSelector(0));
    ES::set_reg(SegmentSelector(0));
    FS::set_reg(SegmentSelector(0));
    GS::set_reg(SegmentSelector(0));

    load_tss(tss_selector);
}
