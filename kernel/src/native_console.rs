//! Bounded native console output. The calling package selects permitted processes.
pub(crate) fn write(fd: u64, address: usize, length: usize) -> i64 {
    if !matches!(fd, 1 | 2) {
        return -9;
    }
    let mut bytes = [0u8; 256];
    let n = length.min(bytes.len());
    if n == 0 {
        return 0;
    }
    if !crate::native_el0_mmu::copy_from_user(address, &mut bytes[..n]) {
        return -14;
    }
    crate::uart_write(&bytes[..n]);
    n as i64
}
