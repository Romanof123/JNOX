"""Independent public admission for all-ready full-grant SplitMix64 windows."""
from itertools import accumulate


def forecast(weights, seed, horizon, width, floors):
    mask = (1 << 64) - 1
    ends = list(accumulate(weights))
    total = ends[-1]
    cutoff = (1 << 64) % total
    state = seed
    winners = []
    for _ in range(horizon):
        while True:
            state = (state + 0x9e3779b97f4a7c15) & mask
            x = state
            x = ((x ^ (x >> 30)) * 0xbf58476d1ce4e5b9) & mask
            x = ((x ^ (x >> 27)) * 0x94d049bb133111eb) & mask
            word = x ^ (x >> 31)
            if word >= cutoff:
                break
        ticket = word % total
        winners.append(next(i for i, end in enumerate(ends) if ticket < end))
    for start in range(horizon - width + 1):
        window = winners[start:start + width]
        for owner, floor in enumerate(floors):
            count = window.count(owner)
            if count < floor:
                raise ValueError(f"lottery interval [{start},{start + width}) owner {owner}: "
                                 f"count {count} < floor {floor}")
