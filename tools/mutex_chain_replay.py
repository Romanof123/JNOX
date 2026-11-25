"""Replay #232 CHAIN RAW 1 from a retained binary UART capture."""
import re
import sys
from pathlib import Path

NA = 2**64 - 1


def require(condition, label):
    if not condition:
        raise ValueError(label)


def replay(path):
    lines = Path(path).read_bytes().replace(b"\r\n", b"\n").splitlines()
    start = lines.index(b"CHAIN RAW 1")
    end = lines.index(b"CHAIN END", start)
    rows = []
    for line in lines[start + 1:end]:
        parts = line.split(b" ")
        require(all(re.fullmatch(b"[0-9a-f]{16}", x) for x in parts[1:]), "wire format")
        rows.append((parts[0], [int(x, 16) for x in parts[1:]]))
    tags = [tag.decode() for tag, _ in rows]
    require(tags == ["H", "T"] + ["S"]*4 + ["E"]*11 + ["R"]*12 + ["P"], "record order")
    h, timer, *tail = [row for _, row in rows]
    stacks, events, returns, payload = tail[:4], tail[4:15], tail[15:27], tail[-1]
    require(h == [0, 11, 12], "terminal header")
    freq, arm, target, iar, ctl, cval = timer
    require(freq > 0 and freq % 1000 == 0 and target == arm + freq//1000
            and cval == target and iar & 1023 == 30 and ctl & 7 == 5, "timer")
    # op, argument, caller, selected, X/Y owners, remaining ready/wait arrival order
    expected = [
        (1,1,0,1,NA,0,1,0), (1,0,1,1,1,0,1,0),
        (1,1,1,3,1,0,0x13,2), (3,0,3,0,1,0,3,0x24),
        (5,0,0,3,1,0,0x13,2), (4,0,3,2,1,0,1,2),
        (4,0,2,0,1,0,0,2), (2,1,0,1,1,1,1,0),
        (2,1,1,1,1,NA,1,0), (2,0,1,1,NA,NA,1,0),
        (4,0,1,0,NA,NA,0,0)]
    for i, (e, want) in enumerate(zip(events, expected)):
        require(len(e) == 28 and tuple(e[2:6]+e[14:16]+e[26:28]) == want, f"event {i}")
        caller, selected = e[4:6]
        require(e[23:25] == [caller+1, selected+1], f"TLS {i}")
        require(e[18] == (0x04030404 if i == 3 else 0x04030202 if 2 <= i <= 6 else 0x04030201), f"ranks {i}")
        for task, frame in [(caller, e[6]), (selected, e[10])]:
            tid, bottom, top, size = stacks[task]
            require(tid == task and bottom <= frame <= top-size and frame % 16 == 0, "stack")
        require(returns[i+1][:2] == e[10:12] and returns[i][2] <= e[0] <= e[1] <= returns[i+1][2], "restore")
    for blocked, resume, result in [(3,4,1), (2,7,0)]:
        require(events[blocked][6:8] == events[resume][10:12] and events[resume][25] == result, "continuation")
    require(events[3][0] <= arm <= events[3][1] and events[4][0] >= target and 0 < events[4][20] < 16384 and events[4][21] == 1, "partial protected work")
    require(events[7][20:22] == [16384,0], "owner completed work")
    require(payload == [4194304, 262144*136]*2 + [1, 16*136], "worker results")


if __name__ == "__main__":
    replay(sys.argv[1])
    print("CHAIN RAW 1 replay passed")
