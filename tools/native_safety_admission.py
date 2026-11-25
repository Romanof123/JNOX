"""Offline timing screen for one native permit qualification capture (Python 3.9+)."""
import argparse
import hashlib
import json
import re
import sys
from pathlib import Path, PurePosixPath


QEMU_SHA256 = "0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8"
QEMU_VERSION = "QEMU emulator version 6.2.0 (Debian 1:6.2+dfsg-2ubuntu6.31)"
PACKAGE_DIGESTS = {
    "fifo-boundary-witness": "489647be1d8881b85932f6e89ae4599421220643740478ed232cb6dc2f05848e",
    "fifo-collision": "28bac3c1a28acac59457d1a640d271c266e88f025b82964df52e207d80980d35",
    "fifo-delayed": "4c6f8ebb2ffec8e44cccca9be5b54c1ec6b1eee8b14deaeb1959d48768126b3f",
    "fifo-healthy-irq": "c411efbaaef42fea026b49582b15cc9ff4a205b40b6fde81cab6122b1a63a94d",
    "fifo-healthy": "8413e0be34b6aa2a59fe14c7477e01de227597deed9b4c15ddad0ffae58c8274",
    "fifo-invalid": "e280c199ff523876ab7244d834327b6786f6d41ffbbdd6d173e6bcae66a4e2d0",
    "fifo-live": "e11b33e215736c3be33015a1fc518cc837840aadb0da2f70b2be1f00f2765710",
    "fifo-lower-work-irq": "87a15e6ba545ad3c5e461254c7e63dad3edce2abb5504be2d4251db20eba5def",
    "fifo-lower-work": "e334d23ee60bd41aad2bff073b88091ade0e7d60a492c68a303e0ddf3c57d8e2",
    "fifo-overrun": "b309a8c57b3aaa752adebaf0b43694d307caa8b4c2f083b31c0c79686674ea18",
    "fifo-saturation": "86fd9f511782ebc9b8eb78881fb5088bab105e5f7c7cd585b9dc65ca2a03d9d4",
    "fifo-silence": "c328d295ababbc3fee5f5f40e75b4c2a0c247039283762b8c080dd95cda4d522",
    "rr-boundary-witness": "57b86b22ae280a9968d23827ad1cd223738c1a485352e4fd6c61e9d5c74a6ce7",
    "rr-collision": "f56f7b10549b87c3c3190c395038365a7f90d1c2e4592eae7ae4adc92fdcd079",
    "rr-delayed": "b7cefc30e39bd41359530c8b5831f2e25409e2da8583f4eaf4a8da77b21fc988",
    "rr-healthy-irq": "33da323d287bd79df374da318753c4aa64e387bc155e25eda038abfdc0c64ab5",
    "rr-healthy": "79992fcc3dc2a88ec02712d0c47426ad76e74efec376554943607b76af576bec",
    "rr-invalid": "976947b30fdc334bd37e03a27ebe44d77551219c1cd2f86297cd166f56866516",
    "rr-live": "8579268673f842551efc6dccd3a4bdbdc7ced693f0194916a4cdb868f8c44c91",
    "rr-lower-work-irq": "2e96a0bfe145e2c63828443723f907bb3bd47bfba086b64e03ed0f56d522a955",
    "rr-lower-work": "e3b0c7b4422ee0039b1d650535734ceeac8e8c33321251066416c8a1d406f4f7",
    "rr-overrun": "e9ad1b10ebdcd1035f171e318f6c14af3618326bc681c444ac98306c0391c6ab",
    "rr-saturation": "a4047b0f0f8f781ba1f61dfc2ba2d90ce4da4304519cb6b548a487c59063bf4a",
    "rr-silence": "4030e29643d3e9300696ce0f6eab5534f011d26f6264e438c4bcad98f6d274b5"
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


# Configuration digests for the 24 first-position #389 packages on fbbb5cea.
# Normalize only source/output locations; all build arguments, slot metadata,
# kernel hash, timing mode and service configuration remain part of the digest.
def deployment_digest(manifest):
    package = manifest["package_argv"]
    locations = [(package[package.index("--out") + 1], "@output")]
    locations.extend((slot["input"]["path"], f"@slot{slot['slot']}") for slot in manifest["slots"])
    replacements = dict(locations)
    if len(replacements) != 4:
        raise ValueError("ambiguous package input/output paths")

    def normalize(item):
        if isinstance(item, str):
            return replacements.get(item, item)
        if isinstance(item, list):
            return [normalize(value) for value in item]
        if isinstance(item, dict):
            return {normalize(key): normalize(value) for key, value in item.items()}
        return item

    return digest(json.dumps(normalize(manifest), sort_keys=True, separators=(",", ":")).encode())


PLATFORM = {"machine": "virt,gic-version=2", "cpu": "cortex-a72", "smp": 1,
            "accel": "tcg", "memory_mib": 128, "clock": "default", "counter_hz": 62500000}


def reviewed_position(path, expected_digest, position, manifest, kernel):
    if path is None or not isinstance(expected_digest, str) or not re.fullmatch("[0-9a-f]{64}", expected_digest):
        raise ValueError("external profile requires both a file and its independently supplied SHA-256")
    raw = path.read_bytes()
    if digest(raw) != expected_digest:
        raise ValueError("external profile digest mismatch")

    def unique(items):
        obj = dict(items)
        if len(items) != len(obj):
            raise ValueError("duplicate external profile key")
        return obj

    spec = json.loads(raw, object_pairs_hook=unique)
    if (set(spec) != {"format", "platform", "qemu", "positions"} or spec["format"] != 1
            or json.dumps(spec["platform"], sort_keys=True) != json.dumps(PLATFORM, sort_keys=True)):
        raise ValueError("unsupported external deployment platform or schema")
    qemu = spec["qemu"]
    if (set(qemu) != {"sha256", "version"} or not re.fullmatch("[0-9a-f]{64}", qemu["sha256"])
            or not isinstance(qemu["version"], str) or not qemu["version"].startswith("QEMU emulator version ")):
        raise ValueError("invalid external emulator identity")
    entry = spec["positions"][position]
    if set(entry) != {"policy", "commit", "manifest_sha256", "kernel_sha256", "slot_sha256", "ram_bytes"}:
        raise ValueError("incomplete or unsupported external position")
    if (entry["policy"] not in ("fifo","rr") or entry["policy"] != manifest["policy"]
            or manifest["board"] != "virt" or manifest.get("tracked_changes") != ""
            or not re.fullmatch("[0-9a-f]{40}", entry["commit"]) or entry["commit"] != manifest["commit"]
            or entry["manifest_sha256"] != deployment_digest(manifest)
            or entry["kernel_sha256"] != kernel
            or entry["slot_sha256"] != [slot["elf_sha256"] for slot in manifest["slots"]]
            or not integer(entry["ram_bytes"], 1) or entry["ram_bytes"] > 128*1024*1024):
        raise ValueError("external position differs from the captured deployment")
    return qemu, entry["ram_bytes"]

def verify_launch(launch, package_directory, qemu_sha256=QEMU_SHA256, qemu_version=QEMU_VERSION):
    argv = launch["argv"]
    if not isinstance(argv, list) or not all(isinstance(word, str) for word in argv) or not argv:
        raise ValueError("invalid QEMU argv")
    output = PurePosixPath(package_directory)
    if output.name != "pkg" or ".." in output.parts:
        raise ValueError("unsupported capture package layout")
    capture = output.parent / "cap"
    expected = [
        "-machine", "virt,gic-version=2", "-cpu", "cortex-a72", "-smp", "1",
        "-accel", "tcg", "-m", "128M", "-nodefaults", "-display", "none",
        "-monitor", "none", "-no-reboot", "-qmp", "stdio",
        "-serial", "file:" + str(capture / "uart.bin"), "-kernel", str(capture / "kernel.elf"),
    ]
    if PurePosixPath(argv[0]).name != "qemu-system-aarch64" or argv[1:] != expected:
        raise ValueError("unsupported QEMU executable, platform, clock, devices or captured paths")
    if (launch["version"] != 1 or launch["qemu_sha256"] != qemu_sha256
            or launch["qemu_version"].splitlines()[0] != qemu_version):
        raise ValueError("unsupported emulator build or launch format")

def integer(value, minimum=0):
    return type(value) is int and value >= minimum


def rows(uart, prefix):
    return [list(map(int, line[len(prefix):].split())) for line in uart.splitlines()
            if line.startswith(prefix)]


def timing_bounds(path, kernel, policy, gaps):
    """Accept externally justified bounds on *response*, never EL0-only WCET."""
    if path is None:
        gaps.append("missing separately justified full-response bounds")
        return {}
    evidence = json.loads(path.read_bytes())
    if evidence["kernel_sha256"] != kernel or evidence["policy"] != policy:
        raise ValueError("bounds target another kernel or policy")
    limits = {}
    for key in ("A", "B", "sender"):
        entry = evidence.get("bounds", {}).get(key)
        if entry is None:
            gaps.append(f"missing justified {key} response bound")
            continue
        if not integer(entry["response_us"], 1) or not str(entry["section"]).strip():
            raise ValueError(f"invalid {key} bound or section")
        artifact = (path.parent / entry["artifact"]).read_bytes()
        if not artifact or digest(artifact) != entry["sha256"]:
            raise ValueError(f"{key} bound reference failed hash verification")
        limits[key] = entry["response_us"]
    return limits


def verify_slot_images(pkg, cap, manifest, launch, receipt, uart, kernel_image):
    """Verify the frozen ELF64 layout, including peers retained only inside the kernel."""
    names = ("user.elf", "native_peer.elf", "native_peer2.elf")
    keys = ("user_elf_sha256", "peer_elf_sha256", "peer2_elf_sha256")
    slots = manifest["slots"]
    expected = [slot["elf_sha256"] for slot in slots]
    boot = re.findall(r"^NATIVE EL0 (?:(PEER[0-9]*) )?ELF ([0-9a-f]{64})\s*$",
                      uart, re.MULTILINE)
    if boot != list(zip(("", "PEER", "PEER2"), expected)):
        raise ValueError("ordered UART ELF identities differ from manifest")
    if launch["user_sha256"] != expected[0] or launch["revision"] != manifest["commit"]:
        raise ValueError("launch executable or revision mismatch")
    # These linked standalone ELF64 files end at their section-header table.
    # Hash complete embedded bytes independently of the digest strings in UART.
    embedded = set()
    for match in re.finditer(b"\x7fELF\x02\x01\x01", kernel_image):
        start = match.start()
        if start == 0 or start + 64 > len(kernel_image):
            continue
        table = int.from_bytes(kernel_image[start+40:start+48], "little")
        entry = int.from_bytes(kernel_image[start+58:start+60], "little")
        count = int.from_bytes(kernel_image[start+60:start+62], "little")
        stop = start + table + entry * count
        if entry == 64 and count and table >= 64 and stop <= len(kernel_image):
            embedded.add(digest(kernel_image[start:stop]))
    for slot, name, key in zip(slots, names, keys):
        wanted = slot["elf_sha256"]
        if slot["elf"] != name or manifest[key] != wanted or wanted not in embedded:
            raise ValueError(f"{name}: package or embedded ELF identity mismatch")
        for parent in (pkg, cap):
            path = parent / name
            recorded = parent == cap and (name == names[0] or name in receipt)
            if recorded and receipt.get(name) != wanted:
                raise ValueError(f"{name}: capture receipt mismatch")
            if (recorded or path.exists()) and digest(path.read_bytes()) != wanted:
                raise ValueError(f"{path}: ELF bytes differ from the package")

def assess(directory, bounds_path, profile_path=None, profile_digest=None):
    pkg, cap = directory / "pkg", directory / "cap"
    manifest_bytes = (pkg / "manifest.json").read_bytes()
    manifest = json.loads(manifest_bytes)
    launch_bytes = (cap / "launch.json").read_bytes()
    launch = json.loads(launch_bytes)
    receipt = json.loads((cap / "session.json").read_bytes())["hashes"]
    uart_bytes = (cap / "uart.bin").read_bytes()
    uart = uart_bytes.decode("ascii")
    kernel_image = (pkg / "kernel.elf").read_bytes()
    kernel = digest(kernel_image)
    policy = manifest["policy"]
    if (receipt["uart.bin"] != digest(uart_bytes) or receipt["launch.json"] != digest(launch_bytes)
            or receipt["kernel.elf"] != kernel or launch["argument_package"] != manifest
            or kernel != manifest["kernel_elf_sha256"]
            or kernel != launch["kernel_sha256"] or kernel != digest((cap / "kernel.elf").read_bytes())):
        raise ValueError("kernel or manifest provenance mismatch")
    failures, gaps, segments = [], [], []
    result = dict(policy=policy, commit=manifest["commit"], kernel_sha256=kernel,
                  manifest_sha256=digest(manifest_bytes), uart_sha256=digest(uart_bytes),
                  failures=failures, missing=gaps, segments=segments)
    case = manifest["slots"][0]["argv"][1]
    profile = f"{policy}-{case}" + ("-irq" if "safety_interference" in manifest else "")
    package_args = manifest["package_argv"]
    package_directory = package_args[package_args.index("--out")+1]
    if profile_path is not None or profile_digest is not None:
        emulator, extent = reviewed_position(profile_path, profile_digest, profile, manifest, kernel)
        result["deployment_profile_sha256"] = profile_digest
        verify_launch(launch, package_directory, emulator["sha256"], emulator["version"])
    else:
        if deployment_digest(manifest) != PACKAGE_DIGESTS.get(profile):
            raise ValueError("unsupported or inconsistent #389 package configuration")
        verify_launch(launch, package_directory)
        extent = 13459520 if "safety_interference" in manifest else 13451264
    ram = (cap / "ram.bin").read_bytes()
    if (launch["ram_base"] != 0x40200000 or launch["ram_bytes"] != extent or len(ram) != extent
            or receipt["ram.bin"] != digest(ram) or launch["probe_sha256"] != receipt["probe.c"]
            or receipt["probe.c"] != digest((cap / "probe.c").read_bytes())):
        raise ValueError("capture memory or probe provenance mismatch")
    qual = manifest.get("safety_qualification", {})
    if (qual.get("controller_phase_offsets_us") != [0, 2500]
            or [qual.get(k) for k in ("syscall", "sender_slot", "controller_slot", "supervisor_slot",
                                       "controller_period_us", "sender_period_us")] != [62, 0, 1, 2, 5000, 10000]):
        raise ValueError("requires the SVC 62 native qualification profile")
    slots = manifest["slots"]
    if [s["slot"] for s in slots] != [0, 1, 2]:
        raise ValueError("requires sender, controller and supervisor slots")
    verify_slot_images(pkg, cap, manifest, launch, receipt, uart, kernel_image)
    if not slots[1]["priority"] > slots[0]["priority"] > slots[2]["priority"]:
        failures.append("controller > sender > supervisor rank requirement")
    if policy == "rr" and not integer(manifest.get("quantum_ticks"), 1):
        raise ValueError("invalid RR quantum")
    result["case_argv"] = [s["argv"] for s in slots]
    result["slot_sha256"] = [s["elf_sha256"] for s in slots]
    try:
        limits = timing_bounds(bounds_path, kernel, policy, gaps)
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        gaps.append(f"rejected timing evidence: {error}")
        limits = {}
    result["submitted_response_bounds_us"] = limits
    counts = rows(uart, "NATIVE QUAL COUNTS ")
    raw = rows(uart, "NATIVE QUAL ROW ")
    if len(counts) != 1 or len(counts[0]) != 4:
        raise ValueError("missing or ambiguous qualification counter metadata")
    hz, attempts, kept, lost = counts[0]
    if hz != 62500000 or kept != len(raw) or attempts != kept + lost:
        raise ValueError("invalid counter metadata")
    if any(len(r) != 13 or r[0] != i for i,r in enumerate(raw)):
        raise ValueError("missing or malformed retained journal rows")
    result.update(counter_hz=hz, retained=kept, lost=lost)
    if rows(uart, "NATIVE SAFETY DROPPED ") != [[0]] or lost:
        gaps.append("incomplete safety or scheduling journal")
    exits = rows(uart, "NATIVE EL0 EXIT PID ")
    if any(len(r)!=2 or r[0] not in (0,1,2) or r[1]!=71+r[0] for r in exits):
        failures.append("unexpected application exit")
    if sorted(exits) != [[0,71],[1,72],[2,73]]:
        gaps.append("incomplete expected process exits")
    # Row numbers below include the append index at [0].
    marks = {}
    for row in raw:
        if row[1] == 2:
            runnable = [s for s in slots if row[7] & (1 << s["slot"])]
            if runnable and row[9] != max(runnable, key=lambda s:s["priority"])["slot"]:
                failures.append(f"dispatch rank at row {row[0]}")
        if row[1] == 1:
            key = tuple(row[2:6])  # pid, job, phase, stage
            if key in marks or row[10] != 0:
                failures.append(f"duplicate or rejected mark {key}")
            marks[key] = row
    origin = marks.get((1,0,1,1))
    if origin is None:
        gaps.append("missing epoch marker")
    for pid,total in ((1,32),(0,8)):
        for index in range(total):
            job,phase = (index//2,1+index%2) if pid==1 else (index,3)
            name = {1:"A",2:"B",3:"sender"}[phase]
            key = (pid,job,phase)
            start, work, end = (marks.get((*key,i)) for i in (1,2,3))
            if any(r is None for r in (start, work, end)):
                gaps.append(f"missing marks {key}")
                continue
            target = start[6]
            if (origin and target != origin[6]+index*(2500 if pid==1 else 10000)*hz//1000000
                    or not target==work[6]==end[6] or not start[0]<work[0]<end[0]
                    or not start[7]<=work[7]<=end[7] or work[7]<target):
                failures.append(f"grid or marker order {key}")
                continue
            # The next stage-1 call blocks this worker. Final jobs park via receive.
            nxt = next((r for r in raw[end[0]+1:] if r[1]==1 and r[2]==pid), None)
            event = None
            if nxt is not None and nxt[5]==1:
                event = next((r for r in raw[nxt[0]+1:] if r[1]==2 and r[2]==62
                              and r[3]==nxt[8] and r[8]==pid),None)
            elif index==total-1 and nxt is None:
                event = next((r for r in raw[end[0]+1:] if r[1]==2 and r[2]==22 and r[8]==pid),None)
            lower = end[7]-target
            upper = None
            if event and event[6] & (1<<pid) and not event[7] & (1<<pid):
                lower=max(lower,event[4]-target)
                upper=event[5]-target
                if upper < lower:
                    raise ValueError(f"inconsistent block bracket {key}")
            else:
                if event and nxt is not None and nxt[4] == 2 and event[7] & (1 << pid):
                    failures.append(f"controller continuation did not block at row {event[0]}")
                gaps.append(f"missing whole-segment blocking boundary {key}")
            ceiling=1000 if pid==1 else 10000
            segments.append(dict(pid=pid,job=job,phase=name,target=target,lower_ticks=lower,
                                 upper_ticks=upper,end_row=end[0],block_row=event[0] if event else None))
            for label,us in (("profile",ceiling),("submitted",limits.get(name))):
                if us is None:
                    continue
                if lower*1000000 > us*hz:
                    failures.append(f"{key} measured lower bound exceeds {label} {us} us")
                elif upper is None or upper*1000000 > us*hz:
                    gaps.append(f"{key} {label} limit unresolved inside bracket")
    gaps.extend(["finite captures do not establish worst-case dispatch, service, blocking or release jitter",
                 "timer reprogramming, other IRQs, supervisor interference and pending carry-in lack a proved envelope",
                 "universal continuation, rank/idle applicability, check gaps and endpoint uncertainty require independent review"])
    result["status"] = "FAIL" if failures else "UNKNOWN/HOLD"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path, help="extracted case with pkg/ and cap/")
    parser.add_argument("--bounds", type=Path)
    parser.add_argument("--deployment-profile", type=Path)
    parser.add_argument("--deployment-profile-sha256")
    args=parser.parse_args()
    try:
        result=assess(args.capture,args.bounds,args.deployment_profile,args.deployment_profile_sha256)
    except (OSError, ValueError, KeyError, IndexError, TypeError, AttributeError) as error:
        result=dict(status="UNKNOWN/HOLD",input_error=str(error))
    print(json.dumps(result,indent=2))
    return 1 if result["status"]=="FAIL" else 2


if __name__ == "__main__":
    sys.exit(main())
