// Queue positions are initial enqueue order. The target is an index into this slice.
// Raw architectural counter ticks throughout (no time-unit conversion).
pub fn bound(
    queue: &[(u8, Option<u64>)],
    target: usize,
    quantum: u64,
    allowance: Option<u64>,
) -> Option<u64> {
    let class = queue.get(target)?.0;
    if quantum == 0 || queue.len() > 4 {
        return None;
    }
    let mut total = allowance?;
    for (position, &(rank, demand)) in queue.iter().enumerate() {
        let contribution = if rank > class {
            demand?
        } else if rank == class && position < target {
            demand?.min(quantum)
        } else {
            continue;
        };
        total = total.checked_add(contribution)?;
    }
    Some(total)
}

pub fn admits(bound: Option<u64>, deadline: u64) -> bool {
    bound.is_some_and(|required| required <= deadline)
}
