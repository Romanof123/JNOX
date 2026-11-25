"""Build a virt kernel and atomically publish its embedded task policy."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile

from build_kernel import cargo_command, read_registry
from task_policy import compile_policy


def kernel_loads(path):
    image = path.read_bytes()
    if image[:7] != b"\x7fELF\x02\x01\x01" or len(image) < 64:
        raise ValueError("invalid kernel ELF header")
    machine, = struct.unpack_from("<H", image, 18)
    offset, = struct.unpack_from("<Q", image, 32)
    size, count = struct.unpack_from("<HH", image, 54)
    if machine != 183 or size != 56 or not count or offset + size * count > len(image):
        raise ValueError("invalid kernel program headers")
    loads = []
    for i in range(count):
        kind, _, pos, va, pa, filesz, memsz, _ = struct.unpack_from("<II6Q", image, offset + i * size)
        if kind != 1:
            continue
        if (va != pa or not 0x40200000 <= pa < pa + memsz <= 0x48000000
                or filesz > memsz or pos + filesz > len(image)):
            raise ValueError("kernel PT_LOAD exceeds virt RAM or file bounds")
        loads.append(dict(address=pa, file_bytes=filesz, memory_bytes=memsz))
    if not loads:
        raise ValueError("kernel has no PT_LOAD")
    return loads


def package(args, root):
    out = args.out.resolve()
    if out.exists():
        raise ValueError(f"output already exists: {out}")
    binary, metadata = compile_policy(args.task_policy.resolve(strict=True))
    digest = hashlib.sha256(binary).hexdigest()
    target = read_registry(root)["aarch64"]
    command = cargo_command(target, args.kernel_profile)
    out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".jnox-policy-", dir=out.parent) as temp:
        work = Path(temp)
        stage = work / "package"
        stage.mkdir()
        policy = stage / "policy.bin"
        policy.write_bytes(binary)
        env = {k: v for k, v in os.environ.items() if not k.startswith(("JNOX_", "CARGO_FEATURE_"))
               and k not in ("CARGO_BUILD_TARGET", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")}
        env.update(JNOX_BOARD="virt", JNOX_ARCH="aarch64", JNOX_PROFILE="task-policy",
                   JNOX_TASK_POLICY_BIN=str(policy), JNOX_TASK_POLICY_SHA256=digest,
                   CARGO_TARGET_DIR=str(work / "target"))
        kernel = root / "kernel"
        log = stage / "build.log"
        with log.open("wb") as stream:
            result = subprocess.run(command, cwd=kernel, env=env, stdout=stream,
                                    stderr=subprocess.STDOUT, timeout=600)
        if result.returncode:
            raise ValueError(log.read_text(encoding="utf-8", errors="replace")[-8000:])
        elf = stage / "kernel.elf"
        shutil.copyfile(work / "target" / target / args.kernel_profile / "jnox-kernel", elf)
        def version(*cmd):
            return subprocess.check_output(cmd, cwd=kernel, env=env, text=True, timeout=20).strip()
        manifest = dict(metadata)
        manifest.update(format=1, profile="task-policy", board="virt", policy_bin="policy.bin",
                        policy_sha256=digest, kernel_elf="kernel.elf",
                        kernel_elf_sha256=hashlib.sha256(elf.read_bytes()).hexdigest(),
                        kernel_loads=kernel_loads(elf), kernel_command=command,
                        kernel_build=dict(target=target, cargo_profile=args.kernel_profile,
                                          policy_sha256=digest, cargo_command=command),
                        rust=version("rustc", "-vV"), cargo=version("cargo", "--version"),
                        commit=version("git", "rev-parse", "HEAD"),
                        tracked_changes=version("git", "status", "--porcelain", "--untracked-files=no"),
                        package_argv=sys.argv[:])
        (stage / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        stage.rename(out)
    print(f"kernel {manifest['kernel_elf_sha256']}  {out / 'kernel.elf'}")
    print(f"policy {digest}  {out / 'policy.bin'}")
