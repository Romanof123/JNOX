fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn section_bytes<'a>(image: &'a [u8], section: &[u8]) -> Option<&'a [u8]> {
    let start = u64_at(section, 24)? as usize;
    let len = u64_at(section, 32)? as usize;
    image.get(start..start.checked_add(len)?)
}

/// The generic ELF plan covers loadable segments. Independently bind the
/// exact allocated SHT_NOTE to the native v1 SDK personality and its RO bytes.
pub(crate) fn native_v1_note(image: &[u8], plan: &LoadPlan<'_>) -> Option<()> {
    const NOTE: &[u8] = b"\x05\0\0\0\x04\0\0\0\x01\0\0\0JNOX\0\0\0\0\x01\0\0\0";
    let mut sizes = image.get(58..64)?.chunks_exact(2);
    let mut word = || Some(usize::from(u16::from_le_bytes(sizes.next()?.try_into().ok()?)));
    let stride = word()?;
    let count = word()?;
    let names_id = word()?;
    if stride != 64 || !(1..=128).contains(&count) || names_id >= count {
        return None;
    }
    let start = usize::try_from(u64_at(image, 40)?).ok()?;
    let table = image.get(start..start.checked_add(count * stride)?)?;
    let names_header = &table[names_id * stride..(names_id + 1) * stride];
    if u32_at(names_header, 4)? != 3 {
        return None;
    }
    let names = section_bytes(image, names_header)?;
    let mut found = false;
    for sh in table.chunks_exact(stride) {
        let tail = names.get(u32_at(sh, 0)? as usize..)?;
        let name = tail.get(..tail.iter().position(|&b| b == 0)?)?;
        if name != b".note.jnox.abi" {
            continue;
        }
        if found || u32_at(sh, 4)? != 7 || u64_at(sh, 8)? != 2 || section_bytes(image, sh)? != NOTE {
            return None;
        }
        let va = u64_at(sh, 16)?;
        let file = u64_at(sh, 24)?;
        found = plan.segments().iter().any(|s| {
            let Some(delta) = va.checked_sub(s.address) else { return false };
            s.permission == Permission::Read && s.file_offset.checked_add(delta) == Some(file)
                && delta.checked_add(NOTE.len() as u64).is_some_and(|end| end <= s.bytes.len() as u64)
        });
        if !found {
            return None;
        }
    }
    found.then_some(())
}
