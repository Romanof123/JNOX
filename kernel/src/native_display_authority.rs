//! Read-only, per-role X11 authorizations provisioned by QEMU virt before boot.
use super::ramfb::{fw_bytes, fw_select};
use crate::native_el0_mmu;

const LIMIT: usize = 256;
const NAMES: [&[u8]; 2] = [b"opt/jnox/xauth-server\0", b"opt/jnox/xauth-client\0"];
const ENOENT: i64 = -2;
const EIO: i64 = -5;
const EACCES: i64 = -13;
const EFAULT: i64 = -14;
const EINVAL: i64 = -22;
const ERANGE: i64 = -34;

struct Snapshot {
    length: Result<usize, i64>,
    bytes: [u8; LIMIT],
}

impl Snapshot {
    const EMPTY: Self = Self { length: Err(ENOENT), bytes: [0; LIMIT] };
}

static mut AUTHORITY: [Snapshot; 2] = [Snapshot::EMPTY, Snapshot::EMPTY];

// Called once on CPU0, with interrupts masked, after installing the EL1-only fw_cfg
// mapping and before entering any process. All later operations read this snapshot.
pub(super) fn initialize() {
    let resources = unsafe { &mut *(&raw mut AUTHORITY) };
    fw_select(0);
    if fw_bytes::<4>() != *b"QEMU" { return; }
    fw_select(0x19);
    let count = u32::from_be_bytes(fw_bytes());
    // Bound boot work on this profile independently of malformed firmware input.
    if count > 256 {
        for r in resources { r.length = Err(EIO); }
        return;
    }

    let mut files = [None, None];
    for _ in 0..count {
        let len = u32::from_be_bytes(fw_bytes()) as usize;
        let key = u16::from_be_bytes(fw_bytes());
        let _reserved = fw_bytes::<2>();
        let name = fw_bytes::<56>();
        if let Some(i) = NAMES.iter().position(|n| name.starts_with(n)) {
            files[i] = Some(if files[i].is_some() || !(0x20..0x4000).contains(&key) {
                Err(EIO)
            } else if !(1..=LIMIT).contains(&len) {
                Err(EINVAL)
            } else {
                Ok((key, len))
            });
        }
    }
    if let [Some(Ok((a, _))), Some(Ok((b, _)))] = files {
        if a == b {
            for r in resources { r.length = Err(EIO); }
            return;
        }
    }
    for (r, file) in resources.iter_mut().zip(files) {
        match file {
            Some(Ok((key, len))) => {
                fw_select(key);
                for byte in &mut r.bytes[..len] { *byte = fw_bytes::<1>()[0]; }
                r.length = Ok(len);
            }
            Some(Err(error)) => r.length = Err(error),
            None => {}
        }
    }
}

pub(super) fn call(pid: usize, op: u64, address: usize, capacity: usize) -> i64 {
    let role = match pid {
        1 => 0,
        4 => 1,
        _ => return EACCES,
    };
    if op > 1 || (op == 0 && (address != 0 || capacity != 0)) { return EINVAL; }
    let resource = unsafe { &*(&raw const AUTHORITY[role]) };
    let len = match resource.length { Ok(n) => n, Err(error) => return error };
    if op == 1 {
        if capacity < len { return ERANGE; }
        if !native_el0_mmu::copy_to_private_process(pid, address, &resource.bytes[..len]) {
            return EFAULT;
        }
    }
    len as i64
}
