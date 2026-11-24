"""Independent work-output and virtual service arithmetic for the ordinary EEVDF guest."""

RING = (11, 7, 23, 5, 19, 3, 29, 13, 2, 31, 17, 37, 41, 43, 47, 53)
MASK = (1 << 64) - 1
MAX_BLOCKS = 1 << 32


def batch_output(task, blocks):
    """Return samples, plain sum and weighted sum using the 112-reading cycle."""
    if task not in (0, 1) or not 0 <= blocks <= MAX_BLOCKS:
        raise ValueError("invalid batch task or block count")
    count = blocks * 64
    cycles, tail = divmod(count, 112)
    value = aux = 0
    for index in range(112):
        reading = RING[(3 * index + 5 * task) % 16]
        multiplicity = cycles + (index < tail)
        value += multiplicity * reading
        aux += multiplicity * reading * (1 + index % 7)
    return count, value & MASK, aux & MASK


def receiver_round(sequence):
    if not 1 <= sequence <= 8:
        raise ValueError("invalid receiver sequence")
    return sum((index + 1) * RING[(index + sequence) % 16] for index in range(16))


def charge(start, used, request, service):
    """Charge a contiguous physical interval, renewing every exhausted request."""
    if request <= 0 or not 0 <= used < request or service < 0:
        raise ValueError("invalid service interval")
    renewals, residual = divmod(used + service, request)
    return start + renewals * request, residual
