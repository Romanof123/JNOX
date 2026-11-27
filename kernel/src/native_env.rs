//! Bounds apply to extra argv and environment records together, including NULs.
pub(crate) const MAX_COUNT: usize = 15;
pub(crate) const MAX_BYTES: usize = 1024;

pub(crate) fn valid(entries: &[&[u8]]) -> bool {
    if entries.len() > MAX_COUNT { return false; }
    for (i, entry) in entries.iter().enumerate() {
        let Some(eq) = entry.iter().position(|&b| b == b'=') else { return false; };
        let name = &entry[..eq];
        if name.is_empty() || !(name[0].is_ascii_alphabetic() || name[0] == b'_')
            || !name.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
            || entry.contains(&0) || core::str::from_utf8(entry).is_err()
            || entries[..i].iter().any(|old| old.starts_with(&entry[..=eq])) {
            return false;
        }
    }
    true
}
