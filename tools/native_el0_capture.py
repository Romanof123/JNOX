#!/usr/bin/env python3
"""Capture one cold native-process boot: NEW_DIR KERNEL_ELF USER_ELF PROBE_C SOURCE_REV."""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import threading
import time

from ordinary_amc_capture import FLAGS, save, sha
from qemu_capture import request


def symbol_address(symbols, name):
    matches = [
        line.split() for line in symbols.splitlines() if line.split()[-1:] == [name]
    ]
    if len(matches) != 1:
        raise ValueError(f"missing or ambiguous symbol {name}")
    return int(matches[0][0], 16)


def command(qemu, root, plain=False):
    return [
        qemu,
        *FLAGS,
        "-qmp",
        "stdio",
        "-serial",
        "file:" + str(root / "uart.bin"),
        *([] if plain else ["-d", "int", "-D", str(root / "exceptions.log")]),
        "-kernel",
        str(root / "kernel.elf"),
    ]


def argument_manifest(args):
    path = args.manifest or args.kernel.parent / "manifest.json"
    requested = [args.arg, args.peer_arg, args.peer2_arg, getattr(args, "peer3_arg", [])]
    if not path.exists() and not args.manifest and not any(requested):
        return None
    manifest = json.loads(path.read_text(encoding="utf-8"))
    if "slots" not in manifest and not args.manifest and not any(requested):
        return None
    slots = manifest["slots"]
    if len(slots) == 4:
        if (manifest.get("app") != "process" or manifest.get("board") != "virt"
                or manifest.get("policy") not in ("fifo", "rr") or "thread_capacity" in manifest):
            raise ValueError("four slots require ordinary virt FIFO/RR processes")
        names = ("user.elf", "native_peer.elf", "native_peer2.elf", "native_peer3.elf")
        for pid, (slot, name) in enumerate(zip(slots, names)):
            if slot.get("slot") != pid or slot.get("elf") != name:
                raise ValueError(f"unexpected packaged ELF for slot {pid}")
            source = args.user if pid == 0 else path.parent / name
            if sha(source) != slot.get("elf_sha256"):
                raise ValueError(f"package ELF digest mismatch for slot {pid}")
    elif not 1 <= len(slots) <= 3:
        raise ValueError("expected one to three slots or four ordinary fixed processes")
    for i, extras in enumerate(requested):
        if extras and (manifest["app"] != "process" or i >= len(slots)
                       or slots[i].get("argv") != ["/bin/app", *extras]):
            raise ValueError(f"slot {i} arguments differ from the packaged kernel")
    if manifest["app"] != "process" or not (args.manifest or len(slots) > 1 or
            any(len(slot.get("argv", ["/bin/app"])) > 1 for slot in slots)):
        return None
    if sha(args.kernel) != manifest["kernel_elf_sha256"] or sha(args.user) != slots[0]["elf_sha256"]:
        raise ValueError("package digest mismatch")
    return manifest


def capture(args, *, command_factory=command):
    if not re.fullmatch("[0-9a-f]{40}", args.revision):
        raise ValueError("full source revision required")
    uart_limit = getattr(args, "uart_limit", None)
    if getattr(args, "timeout", 20) <= 0 or (uart_limit is not None and uart_limit <= 0):
        raise ValueError("capture limits must be positive")
    manifest = argument_manifest(args)
    root = args.root.resolve()
    root.mkdir()
    for src, name in [
        (args.kernel, "kernel.elf"),
        (args.user, "user.elf"),
        (args.probe, "probe.c"),
    ]:
        shutil.copyfile(src, root / name)
    four_fixed = manifest is not None and len(manifest["slots"]) == 4
    if four_fixed:
        package_dir = args.manifest.parent if args.manifest else args.kernel.parent
        for slot in manifest["slots"][1:]:
            name = slot["elf"]
            shutil.copyfile(package_dir / name, root / name)
            if sha(root / name) != slot["elf_sha256"]:
                raise ValueError(f"copied package ELF digest mismatch for {name}")
        if sha(root / "user.elf") != manifest["slots"][0]["elf_sha256"] or sha(root / "kernel.elf") != manifest["kernel_elf_sha256"]:
            raise ValueError("copied package digest mismatch")
    symbols = subprocess.check_output(
        [args.nm, "-nS", str(root / "kernel.elf")], text=True
    )
    base = symbol_address(symbols, "_start")
    limit = symbol_address(symbols, "__stack_top")
    if base != 0x40200000 or not base < limit <= base + (16 << 20) * (len(manifest["slots"]) if manifest else 1):
        raise ValueError("unexpected linked kernel RAM extent")
    qemu = str(Path(shutil.which(args.qemu) or args.qemu).resolve(strict=True))
    cmd = command_factory(qemu, root, getattr(args, "plain", False))
    save(
        root / "launch.json",
        dict(
            version=1,
            **({"argument_package": manifest} if manifest else {}),
            argv=cmd,
            qemu_sha256=sha(Path(qemu)),
            qemu_version=subprocess.check_output([qemu, "--version"], text=True),
            revision=args.revision,
            ram_base=base,
            ram_bytes=limit - base,
            kernel_sha256=sha(root / "kernel.elf"),
            user_sha256=sha(root / "user.elf"),
            probe_sha256=sha(root / "probe.c"),
        ),
    )
    marker = "unobserved"
    terminal_exits = {}
    proc = None
    try:
        with (root / "stderr").open("xb") as err:
            proc = subprocess.Popen(
                cmd,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=err,
                bufsize=0,
            )
            timer = threading.Timer(getattr(args, "timeout", 20), proc.kill)
            timer.start()
            try:
                if "QMP" not in json.loads(proc.stdout.readline()):
                    raise ValueError("missing QMP greeting")

                class Pipe:
                    write = proc.stdin.write
                    readline = proc.stdout.readline

                pipe = Pipe()
                request(pipe, "qmp_capabilities")
                hierarchy_health = False
                while proc.poll() is None:
                    uart = root / "uart.bin"
                    if uart.exists():
                        size = uart.stat().st_size
                        # With no explicit cap, only the hierarchy TIMER producer
                        # gets its larger allowance for 4096 EVENT/TIMER pairs.
                        if size > (uart_limit if uart_limit is not None else 8 << 20):
                            raise ValueError("oversize UART")
                        if size > 1 << 20 and not hierarchy_health:
                            with uart.open("rb") as stream:
                                hierarchy_health = b"\nNATIVE HIERARCHY TIMER " in stream.read(1 << 20)
                            if uart_limit is None and not hierarchy_health:
                                raise ValueError("oversize UART")
                        raw = uart.read_bytes()
                        if not hierarchy_health:
                            hierarchy_health = bool(re.search(rb"(?m)^NATIVE HIERARCHY HEALTH 1 ", raw))
                        exits = {}
                        if manifest and len(manifest["slots"]) > 1:
                            exits = {int(i): int(code) for i, code in re.findall(
                                rb"(?m)^NATIVE EL0 EXIT PID ([0-9]+) (-?[0-9]+)\r?\n", raw)}
                        terminal = re.search(
                            rb"(?m)^NATIVE EL0 (EXIT -?[0-9]+|FAULT [0-9]+)\r?\n",
                            raw,
                        )
                        all_slots = manifest and set(exits) == set(range(len(manifest["slots"])))
                        fault = terminal and terminal[1].startswith(b"FAULT ")
                        if four_fixed:
                            # A single-process exit cannot terminate a four-process capture.
                            terminal = terminal if fault else None
                            all_slots = all_slots and re.search(rb"(?m)^NATIVE FIXED DROPPED [0-9]+\r?\n", raw)
                            terminal_exits = exits
                        if (terminal or all_slots) and hierarchy_health and not fault:
                            # HEALTH precedes the exits. A fragmented final line is not a footer.
                            last_exit = raw.rfind(b"NATIVE EL0 EXIT ")
                            if not re.search(rb"(?m)^NATIVE HIERARCHY HEALTH END 1\r?\n", raw[last_exit:]):
                                time.sleep(0.005)
                                continue
                        if terminal or all_slots:
                            request(pipe, "stop")
                            request(
                                pipe,
                                "pmemsave",
                                val=base,
                                size=limit - base,
                                filename=str(root / "ram.bin"),
                            )
                            marker = terminal[1].decode("ascii") if terminal else f"EXIT {exits[0]}"
                            break
                    time.sleep(0.005)
            finally:
                timer.cancel()
                timer.join()
    except (ValueError, RuntimeError, OSError) as exc:
        marker = str(exc)
    finally:
        if proc is not None:
            if proc.poll() is None:
                proc.terminate()
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
        save(
            root / "session.json",
            dict(
                marker=marker,
                **({"observed_slot_exits": terminal_exits} if four_fixed else {}),
                pid=proc.pid if proc else None,
                hashes={p.name: sha(p) for p in sorted(root.iterdir()) if p.is_file()},
            ),
        )
        print(sha(root / "session.json"))
    return int(marker != f"EXIT {args.expect_exit}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("root", "kernel", "user", "probe"):
        parser.add_argument(name, type=Path)
    parser.add_argument("revision")
    parser.add_argument("--timeout", type=int, default=20, help="wall-clock capture limit in seconds")
    parser.add_argument("--uart-limit", type=int, help="maximum UART bytes (default: 1 MiB, or 8 MiB for hierarchy health)")
    parser.add_argument("--plain", action="store_true", help="capture RAM and UART without external exception tracing")
    parser.add_argument("--expect-exit", type=int, default=37, help="expected signed process exit")
    parser.add_argument("--manifest", type=Path, help="audit packaged ordinary-process arguments")
    for option in ("arg", "peer-arg", "peer2-arg", "peer3-arg"):
        parser.add_argument("--" + option, action="append", default=[],
                            help="check an argument against the already packaged slot")
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--nm", default="aarch64-linux-gnu-nm")
    sys.exit(capture(parser.parse_args()))
