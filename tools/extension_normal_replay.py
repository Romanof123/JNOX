#!/usr/bin/env python3
"""Replay normal worker output and dispatch timing from the same ELF and capture."""
import argparse
from pathlib import Path
import struct
import subprocess


def check(condition, detail):
    if not condition:
        raise ValueError(detail)


def replay(directory, elf):
    table = subprocess.check_output(["aarch64-linux-gnu-nm", "-n", elf], text=True)
    symbols = {f[2]: int(f[0], 16) for line in table.splitlines()
               if len(f := line.split()) == 3}
    audit = (directory / "audit.bin").read_bytes()
    rows = list(struct.iter_unpack("<25Q", audit[56:]))
    count, frequency, origin, quantum, evaluation, first_limit, watchdog = struct.unpack("<7Q", audit[:56])
    results = list(struct.iter_unpack("<5Q", (directory / "trace.bin").read_bytes()))
    check(count == len(rows) == len(results) == 9 and quantum == frequency // 10
          and evaluation == quantum // 4 and first_limit == quantum // 2 and watchdog == 2, "header")
    previous = {}
    names = ["reserved", "fallback", "nominee"]
    temperature, voltage = [18, 23, 21], [3300, 3296, 3304]
    progress = total = reserved = 0
    for index, (row, result) in enumerate(zip(rows, results)):
        period, phase = divmod(index, 3)
        job = (2, 1, 0)[phase]
        begin = origin + index * quantum
        progress += phase != 2
        if phase == 0:
            sample = temperature[period]
            value, mailbox = period + 1, sample
        elif phase == 1:
            sample = temperature[period]
            total += sample
            value, mailbox = total, 0
        else:
            sample = voltage[period]
            reserved += sample
            value, mailbox = reserved, 0
        check(result == (index + 1, job, sample, value, mailbox), f"worker {index}")
        check(row[:10] == (index, (1 if index == 8 else (5, 3, 5)[phase]), (2, 255, 255)[phase],
              (17, 0, 0)[phase], (0, 255, 255)[phase], (0, 6, 7)[phase],
              (2, 2, 0)[phase], job, 1, progress), f"decision {index}")
        if phase == 2:
            check(row[10:15] == (0,) * 5 and row[23] == 255, "reservation bypass")
        else:
            check(row[10] == begin + evaluation <= row[11] <= row[15] and row[23] == 30
                  and symbols["ext_eval_live_work"] <= row[12] < symbols["ext_eval_live_end"]
                  and row[13] != 0 and row[14] > 0, "VM service")
        entry = "ext_" + names[job]
        last_pc, last_frame, last_work = previous.get(job, (symbols[entry], row[21], 0))
        check(row[16] == last_pc and row[21] == last_frame and row[22] > last_work
              and symbols[entry + "_work"] <= row[20] < symbols[entry + "_end"], "context")
        check(begin <= row[15] <= row[17] < begin + first_limit and row[24] == 30
              and row[18] == begin + quantum <= row[19] < row[18] + evaluation, "time")
        previous[job] = row[20:23]
    check(len({record[1] for record in previous.values()}) == 3, "private stacks")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("elf", type=Path)
    args = parser.parse_args()
    replay(args.directory, args.elf)
