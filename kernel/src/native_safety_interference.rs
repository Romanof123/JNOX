//! Optional single-vCPU virt/GICv2 engineering load. CNTP belongs to fixed scheduling.
use core::{arch::asm, ptr};
use super::{context, counter, fail, permit_trace, uart_write, unsigned, CLOCK_DOMAIN, CURRENT};

const ID: u32 = 27;
const LIMIT: usize = 32;
const MASK_LIMIT: usize = 128;
static mut ARMS: [[u64;7];LIMIT] = [[0;7];LIMIT];
static mut IRQS: [[u64;9];LIMIT] = [[0;9];LIMIT];
static mut MASKS: [[u64;6];MASK_LIMIT] = [[0;6];MASK_LIMIT];
static mut ARMED: usize = 0;
static mut RECEIVED: usize = 0;
static mut MASK_COUNT: usize = 0;
static mut MASK_DROPPED: u64 = 0;

fn read_timer() -> (u64,u64,u64) {
    let (v,c,s);
    unsafe { asm!("isb","mrs {v}, cntvct_el0","mrs {c}, cntv_cval_el0","mrs {s}, cntv_ctl_el0",
        v=out(reg)v,c=out(reg)c,s=out(reg)s,options(nostack,preserves_flags)); }
    (v,c,s)
}
fn disable() { unsafe { asm!("msr cntv_ctl_el0,xzr","isb",options(nostack,preserves_flags)); } }

fn arm(ms:u64) {
    let before=counter();
    let (v,_,_)=read_timer();
    let delta=u64::try_from((u128::from(unsafe { CLOCK_DOMAIN.1 })*u128::from(ms)).div_ceil(1000))
        .unwrap_or_else(|_| fail(9));
    let due=v.checked_add(delta).unwrap_or_else(|| fail(9));
    unsafe { asm!("msr cntv_cval_el0,{due}","msr cntv_ctl_el0,{on}","isb",
        due=in(reg)due,on=in(reg)1u64,options(nostack,preserves_flags)); }
    let (_,readback,ctl)=read_timer();
    unsafe {
        ARMS[ARMED]=[before,v,due,readback,ctl,counter(),ms];
        ARMED+=1;
    }
}
pub(super) fn boot() {
    disable();
    let bit=1u32<<ID;
    context::gic_write(context::GICD+0x80,context::gic_read(context::GICD+0x80)&!bit);
    unsafe {ptr::write_volatile((context::GICD+0x400+ID as usize) as *mut u8,0x80);}
    context::gic_write(context::GICD+0x280,bit);
    context::gic_write(context::GICD+0x100,bit);
    unsafe {asm!("dsb sy","isb",options(nostack,preserves_flags));}
    if context::gic_read(context::GICD+0x100)&bit==0 {fail(9);}
    arm(1);
}

pub(super) fn irq(entry:u64,iar:u32,idle:bool) {
    let before=counter();
    let (v,cval,ctl)=read_timer();
    disable();
    unsafe {asm!("dsb sy",options(nostack,preserves_flags));}
    context::gic_write(context::GICC+0x10,iar);
    unsafe {asm!("dsb sy","isb",options(nostack,preserves_flags));}
    let eoi=counter();
    unsafe {
        let i=RECEIVED;
        if i>=ARMED || i>=LIMIT {fail(9);}
        RECEIVED+=1;
        if RECEIVED<LIMIT {arm(5);}
        IRQS[i]=[entry,before,v,cval,ctl,u64::from(iar),if idle {u64::MAX} else {CURRENT as u64},eoi,counter()];
    }
}

pub(super) fn begin_mask() -> Option<(u64,u64)> {
    permit_trace::armed_target(1).map(|due|(due,counter()))
}
pub(super) fn end_mask(sample:Option<(u64,u64)>,op:u64) {
    let end=counter();
    let Some((due,start))=sample else {return};
    if start<=due && due<=end {
        let daif:u64;
        unsafe {
            asm!("mrs {}, daif",out(reg)daif,options(nostack,preserves_flags));
            if MASK_COUNT<MASK_LIMIT {
                MASKS[MASK_COUNT]=[CURRENT as u64,op,start,due,end,daif];
                MASK_COUNT+=1;
            } else {MASK_DROPPED=MASK_DROPPED.saturating_add(1);}
        }
    }
}
fn row(tag:&[u8],i:usize,words:&[u64]) {
    uart_write(tag); uart_write(b" "); unsigned(i as u64);
    for &w in words {uart_write(b" "); unsigned(w);}
    uart_write(b"\n");
}
pub(super) fn report() {
    disable();
    unsafe {
        for i in 0..ARMED {row(b"NATIVE INTERFERENCE ARM",i,&ARMS[i]);}
        for i in 0..RECEIVED {row(b"NATIVE INTERFERENCE IRQ",i,&IRQS[i]);}
        for i in 0..MASK_COUNT {row(b"NATIVE INTERFERENCE MASK",i,&MASKS[i]);}
        row(b"NATIVE INTERFERENCE END",1,&[CLOCK_DOMAIN.1,ARMED as u64,RECEIVED as u64,MASK_COUNT as u64,MASK_DROPPED]);
    }
}
