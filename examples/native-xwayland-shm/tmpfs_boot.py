"""Capture every expected process exit from a serialized tmpfs guest run."""
import hashlib
import json
import re
import subprocess
import time


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path, value):
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")


def boot(qemu, kernel, directory, expected, display):
    directory.mkdir()
    uart = directory / "uart.bin"
    cmd = [str(qemu), "-machine", "virt,gic-version=2", "-cpu", "cortex-a72", "-smp", "1",
           "-accel", "tcg", "-m", "128M", "-nodefaults", "-display", "none",
           "-monitor", "none", "-no-reboot", "-serial", "file:" + str(uart),
           "-kernel", str(kernel)]
    if display:
        cmd += ["-device", "ramfb"]
    write_json(directory / "launch.json", cmd)
    with (directory / "stderr.txt").open("xb") as err:
        proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=err)
        try:
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                raw = uart.read_bytes() if uart.exists() else b""
                if re.search(rb"(?m)^NATIVE EL0 (?:FAULT|FAIL)(?:\s|$)", raw):
                    raise RuntimeError(f"guest fault in {directory.name}")
                rows = re.findall(rb"(?m)^NATIVE EL0 EXIT PID (\d+) (-?\d+)\r?\n", raw)
                observed = {}
                for pid, status in rows:
                    pid, status = int(pid), int(status)
                    if pid in observed or pid not in expected or status != expected[pid]:
                        raise RuntimeError(f"unexpected exit in {directory.name}: PID {pid} = {status}")
                    observed[pid] = status
                if observed == expected:
                    return observed
                if proc.poll() is not None:
                    raise RuntimeError(f"QEMU exited {proc.returncode}; observed {observed}")
                time.sleep(0.1)
            missing = sorted(expected.keys() - observed.keys())
            raise TimeoutError(f"{directory.name} missing terminal rows for PIDs {missing}")
        finally:
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(5)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()


def capture(qemu, kernel, caller, broker, out, mode, roles):
    image_sha = sha256(kernel)
    expected = {pid: status for pid, (_, status) in enumerate(roles)}
    count = 2 if mode == "--tmpfs-cold" else 1
    receipt = {"mode": mode, "kernel_sha256": image_sha, "caller_sha256": sha256(caller),
               "broker_sha256": sha256(broker), "boots": []}
    for index in range(count):
        if sha256(kernel) != image_sha:
            raise RuntimeError("packaged kernel changed before boot")
        directory = out / f"boot-{index+1}"
        observed = boot(qemu, kernel, directory, expected, mode == "--tmpfs-roles")
        if sha256(kernel) != image_sha:
            raise RuntimeError("packaged kernel changed during boot")
        receipt["boots"].append({"capture": directory.name,
            "exits": [{"pid": pid, "role": role, "status": observed[pid]}
                      for pid, (role, _) in enumerate(roles)]})
    write_json(out / "tmpfs-result.json", receipt)
    print(json.dumps(receipt, separators=(",", ":")), flush=True)
