//! Single masked vCPU, protected permit channel. All times are raw CNTPCT ticks.
use super::{counter, fail, loans, native_el0_mmu as mmu, uart_write, unsigned, NativeFrame, CLOCK_DOMAIN};
use core::sync::atomic::{AtomicU64, Ordering};

// Independent simulated hardware register, observable in a terminal RAM capture.
#[unsafe(no_mangle)]
static jnox_virtual_permit_enable: AtomicU64 = AtomicU64::new(0);
const LIMIT: usize = 2048;
static mut LOG: [[u64; 10]; LIMIT] = [[0; 10]; LIMIT];
static mut USED: usize = 0;
static mut DROPPED: u64 = 0;
fn log(kind: u64, data: [u64; 8]) {
    unsafe {
        if USED == LIMIT { DROPPED = DROPPED.saturating_add(1); return; }
        let i = USED;
        LOG[i][0] = kind;
        LOG[i][1] = counter();
        LOG[i][2..].copy_from_slice(&data);
        USED += 1;
    }
}

#[derive(Default)]
struct Session {
    generation: u64, gate: bool, fault: bool, pending: bool, rearmed: bool,
    active: bool, attempts: u8, command: bool, dead: bool, marked: bool,
    sequence: u64, issue: u64, permit: u64, streak: u8, prior: u64,
    snapshot: Option<(u64, [u8; 44])>, released: bool,
    rearm_first: u64, last_check: Option<u64>,
    #[cfg(jnox_native_safety_downstream)]
    downstream_injected: bool,
    #[cfg(jnox_native_safety_downstream)]
    downstream_attempts: u8,
    #[cfg(jnox_native_safety_watchdog)]
    watchdog: CompletionWatchdog,
}
static mut STATE: Option<Session> = None;
fn hz() -> u64 { unsafe { CLOCK_DOMAIN.1 } }
fn fresh(now: u64, issue: u64) -> bool {
    now.checked_sub(issue).is_some_and(|age| u128::from(age)*1000 < u128::from(hz())*20)
}
fn drive(value: u64) {
    let before = counter();
    let old = jnox_virtual_permit_enable.swap(value, Ordering::SeqCst);
    unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)); }
    let actual = jnox_virtual_permit_enable.load(Ordering::SeqCst);
    let after = counter();
    if old != actual { log(2, [before,after,old,actual,0,0,0,0]); }
}
#[cfg(jnox_native_safety_watchdog)]
#[path = "completion_watchdog.rs"]
mod watchdog_policy;
#[cfg(jnox_native_safety_watchdog)]
use watchdog_policy::{CompletionWatchdog, Decision};

#[cfg(jnox_native_safety_watchdog)]
impl Session {
    // Sources 1/30/59/60/61/63 are endpoint exit, physical IRQ or actual SVC.
    // 0x100 + fixed reason means the poll immediately before comparator selection.
    fn watch_record(&self, kind:u64, old:(Option<u64>,Option<u64>), now:u64, source:u64) {
        log(13,[kind,self.generation,old.0.unwrap_or(u64::MAX),old.1.unwrap_or(u64::MAX),
                now,source,self.watchdog.last.unwrap_or(u64::MAX),self.watchdog.due.unwrap_or(u64::MAX)]);
    }

    fn watch_stop(&mut self, kind:u64, now:u64, source:u64) {
        let old = (self.watchdog.last,self.watchdog.due);
        self.watchdog.stop();
        self.watch_inhibit(kind,old,now,source);
    }

    fn watch_inhibit(&mut self, kind:u64, old:(Option<u64>,Option<u64>), now:u64, source:u64) {
        self.fault = true;
        self.pending = false;
        drive(0);
        self.watch_record(kind,old,now,source);
    }

    fn watch_poll(&mut self, now:u64, source:u64) {
        if self.generation == 0 || self.dead { return; }
        let old = (self.watchdog.last,self.watchdog.due);
        match self.watchdog.poll(now) {
            Decision::Clock => self.watch_inhibit(5,old,now,source),
            Decision::Expired => self.watch_inhibit(4,old,now,source),
            _ => {}
        }
    }

    fn watch_complete(&mut self, now:u64, source:u64) -> Result<(),i32> {
        if self.dead { return Err(-110); }
        if self.generation == 0 { return Ok(()); }
        let old = (self.watchdog.last,self.watchdog.due);
        match self.watchdog.complete(now,hz()) {
            Decision::Primed => self.watch_record(2,old,now,source),
            Decision::Renewed => self.watch_record(3,old,now,source),
            Decision::Expired => { self.watch_inhibit(4,old,now,source); return Err(-110); }
            Decision::Clock => { self.watch_inhibit(5,old,now,source); return Err(-110); }
            Decision::Unrepresentable => { self.watch_inhibit(5,old,now,source); return Err(-75); }
            Decision::Latched => return Err(-110),
            Decision::Unchanged => unreachable!(),
        }
        Ok(())
    }
}

#[cfg(jnox_native_safety_watchdog)]
pub(super) fn watchdog_poll(source:u64) -> Option<u64> {
    let s = unsafe { (&mut *(&raw mut STATE)).as_mut().unwrap() };
    s.watch_poll(counter(),source);
    s.watchdog.due
}

impl Session {
    #[inline(never)]
    fn check_at(&mut self, now: u64, _site: u64) {
        #[cfg(any(jnox_native_safety_qualification, jnox_native_safety_boundary))]
        let old_fault = self.fault;
        let clock_fault = self.last_check.is_some_and(|previous| now.checked_sub(previous).is_none_or(|d| d > i64::MAX as u64));
        self.last_check = Some(now);
        if self.pending && !fresh(now,self.rearm_first) { self.pending = false; }
        if !self.fault && (clock_fault || (self.sequence != 0 && !fresh(now,self.issue))) {
            self.fault = true;
            self.pending = false;
            log(3,[now,self.generation,self.sequence,self.issue,0,0,0,0]);
            drive(0);
        }
        // Record the decision's counter operand. The journal samples its own
        // timestamp after the decision and any hardware-register update.
        #[cfg(any(jnox_native_safety_qualification, jnox_native_safety_boundary))]
        log(12, [now, _site, self.generation, self.sequence, self.issue,
                 u64::from(old_fault), u64::from(self.fault), u64::from(self.gate)]);
    }
    fn eligible(&self, now: u64) -> bool {
        #[cfg(jnox_native_safety_watchdog)]
        if self.watchdog.due.is_none_or(|due| now >= due) { return false; }
        self.gate && !self.fault && !self.rearmed && self.streak == 2
            && fresh(now,self.prior) && fresh(now,self.issue)
    }
    #[cfg(jnox_native_safety_downstream)]
    fn inject_downstream(&mut self, pid: usize, generation: u64, entry: u64) -> Result<(), i32> {
        let before = loans::occupancy() as u64;
        let result = if pid != 2 {
            Err(-13)
        } else if self.dead {
            Err(-32)
        } else if generation == 0 || generation != self.generation {
            Err(-116)
        } else if self.gate || !self.marked {
            Err(-16)
        } else if self.downstream_attempts >= 64 {
            Err(-75)
        } else {
            // Charge the generation even when a full queue refuses the fixture.
            self.downstream_attempts += 1;
            loans::inject_invalid_fixture()
        };
        let (status, handle) = match result {
            Ok(handle) => {
                self.downstream_injected = true;
                (0, handle)
            }
            Err(errno) => (i64::from(errno) as u64, 0),
        };
        // The row timestamp bounds the completed attempt; kind 5 carries the current generation.
        log(14, [entry, pid as u64, generation, status, handle, before,
                 loans::occupancy() as u64, u64::from(!self.gate)]);
        result.map(|_| ())
    }
    #[inline(never)]
    fn supervisor(&mut self, cmd:u64, generation:u64) -> Result<(),i32> {
        if self.dead { return Err(-32); }
        if cmd == 1 {
            if generation != self.generation.checked_add(1).ok_or(-75)? { return Err(-116); }
            drive(0);
            loans::invalidate();
            #[cfg(jnox_native_safety_watchdog)]
            let prior = (self.watchdog.last,self.watchdog.due);
            *self = Self { generation, gate:true, ..Self::default() };
            #[cfg(jnox_native_safety_watchdog)]
            self.watch_record(1,prior,counter(),59);
            return Ok(());
        }
        if generation == 0 || generation != self.generation { return Err(-116); }
        match cmd {
            2 => {
                if self.pending || !self.eligible(counter()) { return Err(-11); }
                self.rearm_first = self.prior;
                self.pending = true;
            }
            3 | 4 => {
                if self.marked || !self.gate { return Err(-16); }
                let before = counter();
                self.marked = true;
                if cmd == 3 {
                    self.gate = false;
                    self.pending = false;
                    self.snapshot = None;
                    loans::invalidate();
                }
                log(4,[before,counter(),cmd,generation,self.sequence,self.issue,0,0]);
            }
            _ => return Err(-22),
        }
        Ok(())
    }
    // Keep the snapshot and CRC work out of CHECK and COMPLETE's stack frame.
    #[inline(never)]
    fn commit(&mut self, x:&[u64;31]) -> Result<(),i32> {
        if !self.active || !self.gate || self.fault || !self.released { return Err(-11); }
        let (_, b) = self.snapshot.take().ok_or(-11)?;
        self.released = false;
        let word = |off:usize| u64::from_le_bytes(b[off..off+8].try_into().unwrap());
        let (generation,seq,issue,permit) = (word(8),word(16),word(24),u64::from(b[36]));
        let mut crc = u32::MAX;
        for &byte in &b[..40] {
            crc ^= u32::from(byte);
            for _ in 0..8 { crc = (crc>>1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1)); }
        }
        // Decode first, then recheck the old accepted issue even on a malformed input.
        let now = counter();
        self.check_at(now, 0x102);
        if self.fault { return Err(-110); }
        if !crc != u32::from_le_bytes(b[40..44].try_into().unwrap())
            || b[..8] != [1,1,44,0,1,0,1,1] || b[32..36] != 20u32.to_le_bytes()
            || b[37..40] != [0;3] || permit > 1 || x[1..5] != [generation,seq,issue,permit]
            || generation != self.generation || seq != self.sequence.checked_add(1).ok_or(-75)?
            || (self.sequence != 0 && issue <= self.issue) { return Err(-22); }
        let now = counter();
        self.check_at(now, 0x202);
        if self.fault || !fresh(now,issue) { return Err(-110); }
        let adjacent = self.streak != 0 && issue.checked_sub(self.issue).is_some_and(|delta|
            u128::from(delta)*1000 == u128::from(hz())*10);
        self.prior = self.issue;
        self.streak = if permit == 0 { 0 } else if adjacent { 2 } else { 1 };
        self.sequence = seq;
        self.issue = issue;
        self.permit = permit;
        if permit == 0 { self.pending = false; drive(0); }
        Ok(())
    }
    fn control(&mut self, x:&[u64;31]) -> Result<(),i32> {
        self.check_at(counter(), x[0]);
        match x[0] {
            1 if !self.active => { self.active=true; self.attempts=0; self.command=false; }
            2 => return self.commit(x),
            3 if self.active => { self.pending=false; self.active=false; self.snapshot=None; self.released=false; }
            4 if self.active && !self.command => {
                self.command=true;
                if !core::mem::take(&mut self.pending) || !self.eligible(counter()) { return Err(-11); }
                self.rearmed=true; drive(1);
            }
            _ => return Err(-22),
        }
        Ok(())
    }
}
pub(super) fn boot() {
    if super::COUNT != 3 || super::NATIVE_LOAN_ROUTE != [0,1] || hz()==0 { fail(9); }
    unsafe { STATE = Some(Session::default()); }
    log(1,[hz(),counter(),jnox_virtual_permit_enable.load(Ordering::SeqCst),0,0,0,0,0]);
}
pub(super) fn before_loan(pid:usize,op:u64)->Result<(),i32> {
    let s = unsafe { (&mut *(&raw mut STATE)).as_mut().unwrap() };
    if (matches!(op,33|34) && pid!=0) || (op==35 && pid!=1) || (op==36 && pid>1) { return Err(-13); }
    let allow_loan = s.gate;
    #[cfg(jnox_native_safety_downstream)]
    let allow_loan = allow_loan || (pid == 1 && matches!(op,35|36) && s.downstream_injected && !s.dead);
    if !allow_loan {
        log(9,[pid as u64,op,(-32i64) as u64,s.generation,0,0,0,0]);
        return Err(-32);
    }
    if op == 35 {
        if !s.active || s.attempts == 2 { return Err(-11); }
        s.attempts+=1; s.snapshot=None; s.released=false;
    }
    Ok(())
}
/// Bracket the real publish attempt; wire issue is a producer claim, not the publication time.
pub(super) fn published(handle:u64, captured:Option<[u8;24]>, before:u64, after:u64, status:i32) {
    let bytes = captured.unwrap_or_default();
    let word = |i:usize| u64::from_le_bytes(bytes[i..i+8].try_into().unwrap());
    log(11,[before,after,handle,status as i64 as u64,u64::from(captured.is_some()),word(0),word(8),word(16)]);
}

pub(super) fn after_loan(pid:usize,op:u64,handle:u64,f:&NativeFrame,result:Result<(),i32>, occupancy:usize) {
    let s = unsafe { (&mut *(&raw mut STATE)).as_mut().unwrap() };
    if result.is_ok() && op==35 && f.x[3]==44 {
        let mut bytes=[0;44];
        if !mmu::copy_from_user(f.x[2] as usize,&mut bytes) { fail(9); }
        let mut words=[0u64;8];
        words[0]=f.x[1]; words[1]=s.generation;
        for (i,chunk) in bytes.chunks(8).enumerate() {
            let mut le=[0;8]; le[..chunk.len()].copy_from_slice(chunk);
            words[2+i]=u64::from_le_bytes(le);
        }
        log(7,words);
        s.snapshot=Some((f.x[1],bytes));
    }
    if pid==1 && op==36 && result.is_ok() && s.snapshot.as_ref().is_some_and(|p|p.0==handle) { s.released=true; }
    log(6,[pid as u64,op,result.err().unwrap_or(0) as i64 as u64,occupancy as u64,handle,f.x[1],f.x[3],s.generation]);
}
// INFO and successful fused phase calls use the same protected readback.
#[inline(never)]
fn sample_state(s: &Session, f: &mut NativeFrame) {
    let flags=u64::from(s.gate) | (jnox_virtual_permit_enable.load(Ordering::SeqCst)<<1)
        | (u64::from(s.fault)<<2) | (u64::from(s.pending)<<3);
    #[cfg(jnox_native_safety_watchdog)]
    let flags = flags | (u64::from(s.watchdog.due.is_some())<<4) | (u64::from(s.watchdog.fault)<<5);
    f.x[..7].copy_from_slice(&[0,counter(),hz(),s.generation,flags,s.sequence,s.issue]);
}
// The host observer brackets this service at its call and return.
#[inline(never)]
pub(super) fn service(pid:usize,f:&mut NativeFrame) {
    let s = unsafe { (&mut *(&raw mut STATE)).as_mut().unwrap() };
    let entry=counter();
    let op=f.x[8]; let cmd=f.x[0];
    #[cfg(jnox_native_safety_watchdog)]
    s.watch_poll(entry,op);
    if op==61 {
        sample_state(s, f);
        return;
    }
    let result=match (op,pid) {
        #[cfg(jnox_native_safety_downstream)]
        (59,_) if cmd==5 => s.inject_downstream(pid, f.x[1], entry),
        (59,2)=>s.supervisor(cmd,f.x[1]),
        (60,1)=>s.control(&f.x),
        #[cfg(jnox_native_safety_qualification)]
        (63,1) if matches!(cmd,1|3) => s.control(&f.x),
        #[cfg(jnox_native_safety_qualification)]
        (63,1) => Err(-22),
        _=>Err(-13),
    };
    #[cfg(jnox_native_safety_watchdog)]
    let result = result.and_then(|()| {
        if matches!(op,60|63) && cmd==3 { s.watch_complete(counter(),op) } else { Ok(()) }
    });
    let status=result.err().unwrap_or(0) as i64 as u64;
    log(5,[pid as u64,op,cmd,status,s.generation,s.sequence,s.issue,u64::from(s.streak)]);
    if matches!(op,60|63) && cmd==3 { log(10,[entry,counter(),status,s.generation,s.sequence,s.issue,s.permit,s.streak as u64]); }
    f.x[0]=status;
    #[cfg(jnox_native_safety_qualification)]
    if op == 63 && status == 0 { sample_state(s, f); }
}
pub(super) fn exit(pid:usize) {
    let s = unsafe { (&mut *(&raw mut STATE)).as_mut().unwrap() };
    if !s.dead {
        #[cfg(jnox_native_safety_watchdog)]
        s.watch_stop(6,counter(),1);
        s.dead=true; s.gate=false; s.fault=true; s.pending=false; s.snapshot=None;
        loans::invalidate(); drive(0);
        log(8,[pid as u64,s.generation,s.sequence,s.issue,0,0,0,0]);
    }
}
pub(super) fn report() {
    for i in 0..unsafe {USED} {
        uart_write(b"NATIVE SAFETY EVENT");
        for n in unsafe {LOG[i]} { uart_write(b" "); unsigned(n); }
        uart_write(b"\n");
    }
    uart_write(b"NATIVE SAFETY DROPPED "); unsigned(unsafe {DROPPED}); uart_write(b"\n");
}
