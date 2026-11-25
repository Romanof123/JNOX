//! Restore the original embedded image's writable pages and stack on CPU0.
//! Page descriptors, slot ASID and all other slots' backing remain unchanged.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
struct SegmentIdentity {
    address: u64,
    bytes: usize,
    length: usize,
    memory_size: u64,
    permission: Permission,
}

#[derive(PartialEq, Eq)]
struct ImageIdentity {
    entry: u64,
    segments: [Option<SegmentIdentity>; crate::native_elf::MAX_LOADS],
}

impl ImageIdentity {
    fn from_plan(plan: &LoadPlan<'_>) -> Self {
        let mut segments = [None; crate::native_elf::MAX_LOADS];
        for (dest, s) in segments.iter_mut().zip(plan.segments()) {
            *dest = Some(SegmentIdentity {
                address: s.address, bytes: s.bytes.as_ptr().addr(), length: s.bytes.len(),
                memory_size: s.memory_size, permission: s.permission,
            });
        }
        Self { entry: plan.entry, segments }
    }
}

static mut IMAGES: [Option<ImageIdentity>; SPACE_CAPACITY] = [const { None }; SPACE_CAPACITY];
static mut PRIVILEGED_ROOT: Table = Table::EMPTY;
static mut PRIVILEGED_LOW: Table = Table::EMPTY;

pub(super) unsafe fn remember(pid: usize, plan: &LoadPlan<'_>) {
    unsafe { IMAGES[pid] = Some(ImageIdentity::from_plan(plan)); }
}

pub(super) unsafe fn prepare_kernel_root() {
    unsafe {
        // ASID zero has no EL0 mappings. Keep the same privileged code,
        // data, exception stack and device aliases even in a one-slot package.
        link(&raw mut PRIVILEGED_ROOT, LINK_START, 30, &raw mut HIGH);
        link(&raw mut PRIVILEGED_ROOT, 0, 30, &raw mut PRIVILEGED_LOW);
        link(&raw mut PRIVILEGED_LOW, crate::UART0, 21, &raw mut UART);
        link(&raw mut PRIVILEGED_LOW, crate::context::GICD, 21, &raw mut GIC);
    }
}

unsafe fn set_root(ttbr: u64) {
    unsafe {
        asm!("dsb sy", "msr ttbr0_el1, {}", "isb", in(reg) ttbr,
             options(nostack, preserves_flags));
    }
}

/// Called only after the scheduler has captured any discarded/completed
/// product and revoked the previous generation's frame, wait and return.
/// The plan must refer to the same immutable embedded ELF used at boot.
/// The startup tail must be kernel-owned and disjoint from this slot's backing.
/// Caller serializes the transition on masked CPU0 and sets fresh frame/TLS.
pub(crate) unsafe fn amc_reload(pid: usize, plan: &LoadPlan<'_>, stack_tail: &[u8])
    -> Result<(), &'static str>
{
    let target = space_id(pid).ok_or("uninstalled AMC slot")?;
    let prior = active().ok_or("unknown AMC address space")?;
    if !irq_masked() { return Err("unmasked AMC reload"); }
    validate(plan, stack_tail)?;
    let installed = unsafe { &*(&raw const IMAGES[pid]) };
    if installed.as_ref() != Some(&ImageIdentity::from_plan(plan)) {
        return Err("AMC slot ELF changed");
    }
    let prior_root = root_id(prior);
    unsafe {
        if prior_root == target {
            set_root((&raw const PRIVILEGED_ROOT).addr() as u64);
        }
        // The mappings have normal noncacheable memory and D/I caches off.
        // RX and RO are pristine by construction: EL0 never has a writable
        // alias and this interface never modifies those bytes. Reset every
        // writable page, including ELF zero-fill and accessible page padding.
        let space = &mut *(&raw mut SPACES[pid]);
        let image = space.image_ram.0.as_mut_ptr();
        for segment in plan.segments().iter().filter(|s| s.permission == Permission::ReadWrite) {
            let dest = image.add(segment.address as usize - IMAGE_LO);
            let extent = (segment.page_end - segment.address) as usize;
            ptr::copy_nonoverlapping(segment.bytes.as_ptr(), dest, segment.bytes.len());
            ptr::write_bytes(dest.add(segment.bytes.len()), 0, extent - segment.bytes.len());
        }
        let stack = &mut space.stack_ram.0;
        let tail_start = stack.len() - stack_tail.len();
        stack[..tail_start].fill(0);
        stack[tail_start..].copy_from_slice(stack_tail);
        // Publish all contents before any process return. Translation tags
        // and physical page ownership were never repurposed.
        if prior_root == target {
            set_root(prior_root);
        } else {
            asm!("dsb sy", "isb", options(nostack, preserves_flags));
        }
    }
    if active() != Some(prior) { return Err("AMC reload TTBR readback"); }
    Ok(())
}
