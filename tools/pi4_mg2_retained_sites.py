"""Read the published MG2 first archive. Extract linked causal observation sites and actual per-CPU traces."""
import argparse
import hashlib
import json
import re
import struct
from zipfile import ZipFile


def sha(data):
    return hashlib.sha256(data).hexdigest()


def one(values, label):
    if len(values) != 1:
        raise ValueError(f"{label}: expected one, got {len(values)}")
    return values[0]


def inspect(archive):
    if not __debug__:
        raise ValueError("Run the audit with Python assertions enabled")
    with ZipFile(archive) as z:
        sums = z.read("SHA256SUMS").decode().splitlines()
        first_six = json.loads(z.read("first-six.json"))
        for line in sums:
            digest, path = line.split("  ", 1)
            if sha(z.read(path)) != digest:
                raise ValueError(path)
        result = {
            "archive_sha256": sha(archive.read_bytes()),
            "checked_files": len(sums),
            "regimes": {},
        }
        for regime in ("global", "partitioned", "clustered"):
            frozen = json.loads(z.read(f"{regime}/freeze.json"))
            dis = z.read(f"{regime}/disassembly.txt")
            raw = z.read(f"{regime}/kernel8.img")
            for name in ("disassembly.txt", "kernel8.img", "kernel.elf"):
                if sha(z.read(f"{regime}/{name}")) != frozen["hashes"][name]:
                    raise ValueError(f"{regime}/{name}")
            symbols = {}
            current = None
            for line in dis.decode().splitlines():
                heading = re.fullmatch(r"([0-9a-f]+) <([^>]+)>:", line)
                if heading:
                    current = heading[2]
                    symbols[current] = (int(heading[1], 16), [])
                ins = re.fullmatch(r"\s*([0-9a-f]+):\s+([0-9a-f]{8})\s+(.+)", line)
                if ins and current:
                    symbols[current][1].append(
                        (int(ins[1], 16), int(ins[2], 16), ins[3])
                    )

            def function(piece):
                return one([v for name, v in symbols.items() if piece in name], piece)

            def site(function_part, expr):
                pc, word, _ = one(
                    [i for i in function(function_part)[1] if re.search(expr, i[2])],
                    expr,
                )
                if struct.unpack_from("<I", raw, pc - 0x80000)[0] != word:
                    raise ValueError(f"raw opcode mismatch {pc:#x}")
                return pc

            encoded = {
                pc: word for _, items in symbols.values() for pc, word, _ in items
            }
            inner = function("pi4_migration_input_loop")[1]
            loop_lo = one(
                [
                    i[0]
                    for i in inner
                    if re.match(r"ldr\s+x8, \[x21\]", i[2])
                    and any(re.search(rf"b.cc\s+{i[0]:x}\b", j[2]) for j in inner)
                ],
                "loop back-edge destination",
            )
            loop_hi = one(
                [i[0] + 4 for i in inner if re.search(rf"b.cc\s+{loop_lo:x}\b", i[2])],
                "loop back-edge",
            )
            sites = {
                "m_entry": symbols["pi4_migration_m"][0],
                "p1_entry": symbols["pi4_migration_p1_open"][0],
                "m_sparse_checkpoint_log_call": site(
                    "10checkpoint17", r"\bbl\s+[0-9a-f]+ <.*pi4_migration_log5event"
                ),
                "m_first_milestone_release": site("10checkpoint17", r"^stlrb\s"),
                "m_svc": one(
                    [
                        i[0]
                        for i in symbols["pi4_migration_m"][1]
                        if re.match(r"^svc\s", i[2])
                    ],
                    "M SVC",
                ),
                "sgi_mmio_write": site("11pi4_smp_bsp6signal", r"^str\s+w0, \[x8\]$"),
                "sgi_post_dsb_isb_return": site("11pi4_smp_bsp6signal", r"^ret\s*$"),
                "p1_idle_entry": function("13pi4_migration4idle17")[0],
            }
            p1 = symbols["pi4_migration_p1_open"][1]
            p1_svc = one([i[0] for i in p1 if re.match(r"^svc\s", i[2])], "p1 svc")
            # Both first-positive/follow-up-positive paths fall through to the instruction before SVC.
            path_pc = p1_svc - 4
            assert any(
                i[0] == path_pc and re.match(r"mov\s+w0, #0x2", i[2]) for i in p1
            )
            assert any(re.search(rf"cbnz\s+w10, {path_pc:x}\b", i[2]) for i in p1)
            sites["p1_observed_milestone_success_path"] = path_pc
            sites["p1_svc"] = p1_svc
            for pc in [*sites.values(), *range(loop_lo, loop_hi, 4)]:
                assert struct.unpack_from("<I", raw, pc - 0x80000)[0] == encoded[pc]
            cases = []
            for ordinal in (1, 2):
                folder = f"{regime}/first-{ordinal}"
                original = one(
                    [
                        c
                        for c in first_six
                        if c["case"] == regime and c["position"] == ordinal
                    ],
                    "declared first position",
                )
                assert original["strict_loop"] == [[loop_lo, loop_hi]]
                assert (
                    sha(z.read(f"{folder}/session.json"))
                    == original["public_session_sha256"]
                )
                threads = json.loads(z.read(f"{folder}/threads.json"))
                sessions = json.loads(z.read(f"{folder}/session.json"))
                output = []
                for cpu in (0, 1):
                    tid = one(
                        [k for k, v in threads.items() if v == f"CPU {cpu}/TCG"],
                        "vCPU ID",
                    )
                    name = f"observer-{tid}.log"
                    blob = z.read(f"{folder}/{name}")
                    assert sha(blob) == sessions["hashes"][name]
                    content = blob.decode()
                    erets = [
                        int(x, 16)
                        for x in re.findall(
                            r"Exception return from AArch64 EL\d to AArch64 EL1 PC (0x[0-9a-f]+)",
                            content,
                        )
                    ]
                    irqs = content.count("[IRQ]")
                    assert (
                        irqs == 0
                        and content.count(f"Taking exception 2 [SVC] on CPU {cpu}\n")
                        == 1
                    )
                    elr = int(
                        one(re.findall(r"with ELR (0x[0-9a-f]+)", content), "SVC ELR"),
                        16,
                    )
                    spsr = int(
                        one(
                            re.findall(r"with SPSR (0x[0-9a-f]+)", content),
                            "original SPSR",
                        ),
                        16,
                    )
                    assert spsr & 0x80 == 0 and elr == [sites["m_svc"], p1_svc][cpu] + 4
                    expected = (
                        [sites["m_entry"]]
                        if cpu == 0
                        else [sites["p1_entry"], sites["p1_idle_entry"]]
                    )
                    assert erets[1:] == expected
                    output.append(
                        {
                            "cpu": cpu,
                            "trace_sha256": sha(blob),
                            "eret_pcs": list(map(hex, erets)),
                            "svc_elr": hex(elr),
                            "interrupted_spsr": hex(spsr),
                            "irq_entries": irqs,
                        }
                    )
                uart = z.read(f"{folder}/uart.bin")
                assert sha(uart) == sessions["hashes"]["uart.bin"] and uart.endswith(
                    b"PI4 MIG FAIL 000000000000001d\n"
                )
                cases.append(
                    {
                        "first_position": ordinal,
                        "uart_sha256": sha(uart),
                        "cpu_traces": output,
                    }
                )
            result["regimes"][regime] = {
                "source": frozen["source"],
                "freeze_image_hashes": frozen["hashes"],
                "strict_loop_half_open": list(map(hex, (loop_lo, loop_hi))),
                "sites": {k: hex(v) for k, v in sites.items()},
                "firsts": cases,
            }
        return result


if __name__ == "__main__":
    from pathlib import Path

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    args = parser.parse_args()
    print(json.dumps(inspect(args.archive), indent=2))
