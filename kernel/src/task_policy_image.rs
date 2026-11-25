//! Decode the linked policy package and load each address space once at cold boot.
use crate::native_elf::{self, LoadPlan, Permission};
use core::mem::MaybeUninit;

include!(concat!(env!("OUT_DIR"), "/task_policy_sizes.rs"));
static IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/task_policy.bin"));

const HEADER: usize = 16;
const APP_BYTES: usize = 48;
const TASK_BYTES: usize = 64;

pub(crate) struct App {
    pub name: &'static str,
    pub first_task: usize,
    pub count: usize,
    elf_offset: usize,
    elf_length: usize,
}

pub(crate) struct Task {
    pub name: &'static str,
    pub owner: usize,
    pub slot: usize,
    pub entry: u64,
    pub period_ms: u32,
    pub priority: u32,
    pub deadline_ms: u32,
    pub wd_ms: u32,
}

fn word(record: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(record[offset..offset + 4].try_into().expect("sized policy record"))
}

fn name(record: &'static [u8]) -> Option<&'static str> {
    let field = &record[..32];
    let end = field.iter().position(|&c| c == 0)?;
    if end == 0 || !(field[0].is_ascii_alphabetic() || field[0] == b'_')
        || !field[..end].iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
        || field[end..].iter().any(|&c| c != 0)
    {
        return None;
    }
    core::str::from_utf8(&field[..end]).ok()
}

pub(crate) fn app(pid: usize) -> Option<App> {
    if pid >= APP_COUNT { return None; }
    let start = HEADER + pid * APP_BYTES;
    let row = IMAGE.get(start..start + APP_BYTES)?;
    Some(App {
        name: name(row)?,
        elf_offset: word(row, 32) as usize,
        elf_length: word(row, 36) as usize,
        first_task: word(row, 40) as usize,
        count: word(row, 44) as usize,
    })
}

pub(crate) fn task(id: usize) -> Option<Task> {
    if id >= TASK_COUNT { return None; }
    let start = HEADER + APP_COUNT * APP_BYTES + id * TASK_BYTES;
    let row = IMAGE.get(start..start + TASK_BYTES)?;
    Some(Task {
        name: name(row)?,
        owner: word(row, 32) as usize,
        slot: word(row, 36) as usize,
        entry: u64::from_le_bytes(row[40..48].try_into().ok()?),
        period_ms: word(row, 48),
        priority: word(row, 52),
        deadline_ms: word(row, 56),
        wd_ms: word(row, 60),
    })
}

// The number of decoded ELFs can vary independently of the EL1 stack size.
static mut PLANS: [MaybeUninit<LoadPlan<'static>>; APP_COUNT] =
    [const { MaybeUninit::uninit() }; APP_COUNT];

/// Boot CPU only, with interrupts masked and before any address space is live.
pub(crate) unsafe fn install() -> Result<(), &'static str> {
    let h = IMAGE.get(..HEADER).ok_or("policy header")?;
    if &h[..4] != b"JNP1" || word(h, 4) != 1 || word(h, 8) as usize != APP_COUNT
        || word(h, 12) as usize != TASK_COUNT {
        return Err("policy header fields");
    }

    let mut offset = HEADER + APP_COUNT * APP_BYTES + TASK_COUNT * TASK_BYTES;
    let mut first = 0;
    let mut max_count = 0;
    let plans = (&raw mut PLANS).cast::<LoadPlan<'static>>();
    for pid in 0..APP_COUNT {
        let a = app(pid).ok_or("policy app row")?;
        if a.elf_offset != offset || a.first_task != first || a.count == 0
            || a.count > MAX_TASKS_PER_APP || a.count > TASK_COUNT - first
            || (0..pid).any(|other| app(other).is_some_and(|v| v.name == a.name)) {
            return Err("policy app layout");
        }
        offset = offset.checked_add(a.elf_length).ok_or("policy ELF length")?;
        let elf = IMAGE.get(a.elf_offset..offset).ok_or("policy ELF range")?;
        let plan = native_elf::plan(elf, 0x0040_0000..0x0080_0000).map_err(|_| "policy ELF")?;
        let perms = [Permission::ReadExecute, Permission::Read, Permission::ReadWrite];
        if plan.entry != 0x0040_0000 || !plan.segments().iter().map(|s| s.permission).eq(perms)
            || crate::native_el0::native_v1_note(elf, &plan).is_none() {
            return Err("policy native ABI");
        }
        for slot in 0..a.count {
            let t = task(first + slot).ok_or("policy task row")?;
            let end = t.entry.checked_add(4).ok_or("policy entry overflow")?;
            if t.owner != pid || t.slot != slot || t.entry % 4 != 0
                || !(1..=255).contains(&t.priority)
                || !(1..=t.period_ms).contains(&t.deadline_ms)
                || (first..first + slot).any(|id| task(id).is_some_and(|v| v.name == t.name))
                || !plan.segments().iter().any(|s| s.permission == Permission::ReadExecute
                    && t.entry >= s.address && end <= s.address + s.bytes.len() as u64) {
                return Err("policy task fields");
            }
        }
        first += a.count;
        max_count = max_count.max(a.count);
        unsafe { plans.add(pid).write(plan); }
    }
    if first != TASK_COUNT || max_count != MAX_TASKS_PER_APP || offset != IMAGE.len() {
        return Err("policy package extent");
    }
    // All plans now refer into the same immutable package validated above.
    let plans = unsafe { core::slice::from_raw_parts(plans, APP_COUNT) };
    unsafe { crate::native_el0_mmu::install_processes(plans, &[&[]; APP_COUNT]) }
}
