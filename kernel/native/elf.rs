//! Allocation-free native AArch64 ELF layout inspection. All addresses are guest
//! virtual addresses. No mapping, relocation, authentication or execution occurs.
use core::ops::Range;

pub const PAGE: u64 = 4096;
pub const MAX_LOADS: usize = 8;
const HEADER: usize = 64;
const PHDR: usize = 56;
const MAX_HEADERS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    UserWindow,
    Header,
    Identification,
    Target,
    ProgramTable,
    HeaderType,
    StackHeader,
    SegmentCount,
    Permissions,
    Alignment,
    FileRange,
    MemoryRange,
    SegmentOrderOrOverlap,
    Entry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Read,
    ReadExecute,
    ReadWrite,
}

#[derive(Clone, Copy, Debug)]
pub struct Segment<'a> {
    pub address: u64,
    pub file_offset: u64,
    pub bytes: &'a [u8],
    /// Bytes reserved by p_memsz, including file bytes.
    pub memory_size: u64,
    /// Exclusive upper bound of pages to map, including zero-filled padding.
    pub page_end: u64,
    pub permission: Permission,
}

#[derive(Debug)]
pub struct LoadPlan<'a> {
    pub entry: u64,
    segments: [Segment<'a>; MAX_LOADS],
    used: usize,
}

impl<'a> LoadPlan<'a> {
    pub fn segments(&self) -> &[Segment<'a>] {
        &self.segments[..self.used]
    }
}

// All offsets below are constants in a previously sized ELF or program header.
fn u16_at(buf: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([buf[i], buf[i + 1]])
}
fn u32_at(buf: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(buf[i..i + 4].try_into().expect("fixed header field"))
}
fn u64_at(buf: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(buf[i..i + 8].try_into().expect("fixed header field"))
}

pub fn plan(image: &[u8], window: Range<u64>) -> Result<LoadPlan<'_>, Error> {
    if window.start >= window.end || window.start % PAGE != 0 || window.end % PAGE != 0 {
        return Err(Error::UserWindow);
    }
    let h = image.get(..HEADER).ok_or(Error::Header)?;
    if &h[..9] != b"\x7fELF\x02\x01\x01\x00\x00" {
        return Err(Error::Identification);
    }
    if u16_at(h, 16) != 2 || u16_at(h, 18) != 183 || u32_at(h, 20) != 1 || u32_at(h, 48) != 0 {
        return Err(Error::Target);
    }
    if usize::from(u16_at(h, 52)) != HEADER || usize::from(u16_at(h, 54)) != PHDR {
        return Err(Error::ProgramTable);
    }
    let count = usize::from(u16_at(h, 56));
    let offset = usize::try_from(u64_at(h, 32)).map_err(|_| Error::ProgramTable)?;
    if !(1..=MAX_HEADERS).contains(&count) || offset < HEADER {
        return Err(Error::ProgramTable);
    }
    let end = offset
        .checked_add(count * PHDR)
        .ok_or(Error::ProgramTable)?;
    let table = image.get(offset..end).ok_or(Error::ProgramTable)?;
    let entry = u64_at(h, 24);
    let instruction_end = entry.checked_add(4).ok_or(Error::Entry)?;
    let empty = Segment {
        address: 0,
        file_offset: 0,
        bytes: &[],
        memory_size: 0,
        page_end: 0,
        permission: Permission::Read,
    };
    let mut result = LoadPlan {
        entry,
        segments: [empty; MAX_LOADS],
        used: 0,
    };
    let mut previous_end = window.start;
    let mut executable_entry = false;
    let mut stack = false;
    for ph in table.chunks_exact(PHDR) {
        let flags = u32_at(ph, 4);
        let file_offset = u64_at(ph, 8);
        let address = u64_at(ph, 16);
        let file_size = u64_at(ph, 32);
        let memory_size = u64_at(ph, 40);
        let align = u64_at(ph, 48);
        match u32_at(ph, 0) {
            0 => continue,
            0x6474e551 => {
                if stack || flags & !6 != 0 || file_size != 0 || memory_size != 0 {
                    return Err(Error::StackHeader);
                }
                stack = true;
                continue;
            }
            1 => (),
            _ => return Err(Error::HeaderType),
        }
        if result.used == MAX_LOADS {
            return Err(Error::SegmentCount);
        }
        let permission = match flags {
            4 => Permission::Read,
            5 => Permission::ReadExecute,
            6 => Permission::ReadWrite,
            _ => return Err(Error::Permissions),
        };
        if align < PAGE
            || !align.is_power_of_two()
            || address % PAGE != 0
            || file_offset % PAGE != 0
            || address % align != file_offset % align
        {
            return Err(Error::Alignment);
        }
        if memory_size == 0 || file_size > memory_size {
            return Err(Error::MemoryRange);
        }
        let start = usize::try_from(file_offset).map_err(|_| Error::FileRange)?;
        let file_end = file_offset
            .checked_add(file_size)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(Error::FileRange)?;
        let bytes = image.get(start..file_end).ok_or(Error::FileRange)?;
        let end = address.checked_add(memory_size).ok_or(Error::MemoryRange)?;
        let page_end = end.checked_add(PAGE - 1).ok_or(Error::MemoryRange)? & !(PAGE - 1);
        if address < window.start || page_end > window.end {
            return Err(Error::MemoryRange);
        }
        if address < previous_end {
            return Err(Error::SegmentOrderOrOverlap);
        }
        previous_end = page_end;
        // file_size <= memory_size and address + memory_size was checked.
        if permission == Permission::ReadExecute
            && entry >= address
            && instruction_end <= address + file_size
        {
            executable_entry = true;
        }
        result.segments[result.used] = Segment {
            address,
            file_offset,
            bytes,
            memory_size,
            page_end,
            permission,
        };
        result.used += 1;
    }
    if result.used == 0 {
        return Err(Error::SegmentCount);
    }
    if !executable_entry || entry % 4 != 0 {
        return Err(Error::Entry);
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;

    // One RX page with four file bytes followed by twelve BSS bytes.
    fn fixture() -> std::vec::Vec<u8> {
        let mut image = std::vec![0u8;4100];
        image[..9].copy_from_slice(b"\x7fELF\x02\x01\x01\x00\x00");
        for (i, n) in [(16, 2u16), (18, 183), (52, 64), (54, 56), (56, 1)] {
            image[i..i + 2].copy_from_slice(&n.to_le_bytes());
        }
        for (i, n) in [(20, 1u32), (64, 1), (68, 5)] {
            image[i..i + 4].copy_from_slice(&n.to_le_bytes());
        }
        for (i, n) in [
            (24, 0x400000u64),
            (32, 64),
            (72, 4096),
            (80, 0x400000),
            (96, 4),
            (104, 16),
            (112, 4096),
        ] {
            image[i..i + 8].copy_from_slice(&n.to_le_bytes());
        }
        image
    }
    #[test]
    fn load_and_clear_lengths() {
        let image = fixture();
        let p = plan(&image, 0x400000..0x401000).unwrap();
        let s = &p.segments()[0];
        assert_eq!(
            (p.entry, s.bytes.len(), s.memory_size, s.page_end),
            (0x400000, 4, 16, 0x401000)
        );
    }
    #[test]
    fn entry_requires_file_backed_instruction() {
        let mut image = fixture();
        image[24..32].copy_from_slice(&0x400004u64.to_le_bytes());
        assert_eq!(plan(&image, 0x400000..0x401000).unwrap_err(), Error::Entry);
    }
    #[test]
    fn permissions_and_user_window() {
        let mut image = fixture();
        assert_eq!(
            plan(&image, 0x401000..0x402000).unwrap_err(),
            Error::MemoryRange
        );
        image[68] = 7;
        assert_eq!(
            plan(&image, 0x400000..0x401000).unwrap_err(),
            Error::Permissions
        );
    }
}
