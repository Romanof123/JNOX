/// Single-core permit qualification SVC. Phase 1/2 = controller A/B, 3 = sender,
/// 4 = supervisor. Stage 1 arms one absolute sleep, 2 records first execution,
/// 3 records completion after the segment's work. The target is in raw ticks.
#[inline]
pub fn mark(job: u64, phase: u64, stage: u64, target: u64) -> Result<(), i64> {
    let status: i64;
    unsafe {
        core::arch::asm!("svc #0x4a01", in("x8") 62u64, inout("x0") job => status,
                         in("x1") phase, in("x2") stage, in("x3") target, options(nostack));
    }
    if status == 0 { Ok(()) } else { Err(status) }
}
