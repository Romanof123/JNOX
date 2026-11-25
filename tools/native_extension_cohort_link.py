#!/usr/bin/env python3
"""Internal target linker for the #349 source cohort. Called only by the packager."""
import json
import os
from pathlib import Path
import shlex
import shutil
import sys

# The linker is invoked through a symlink named rust-lld.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from native_extension_cohort import ROOT, TARGET, digest, require, run_logged, save


def link(root, spec, audit):
    target = root/"ordinary/target"
    require(Path.cwd().resolve() == ROOT/"kernel" and os.environ.get("JNOX_NATIVE_POLICY") == "extension"
            and Path(os.environ["CARGO_TARGET_DIR"]).resolve() == target, "unexpected Cargo invocation")
    blobs = audit/"blobs"
    blobs.mkdir()
    inputs, responses = [], []

    def retain(path):
        path = path.resolve(strict=True)
        require(path.is_file(), "non-file input")
        h = digest(path)
        dest = blobs/h
        if not dest.exists():
            shutil.copyfile(path, dest)
            require(digest(dest) == h, "input changed during archival")
        return h

    def expand(args, depth=0):
        require(depth < 8, "nested response files")
        for arg in args:
            if arg.startswith("@"):
                p = Path(arg[1:]).resolve(strict=True)
                responses.append(dict(path=str(p), sha256=retain(p)))
                yield from expand(shlex.split(p.read_text(encoding="utf-8")), depth+1)
            else:
                yield arg

    raw = sys.argv[1:]
    save(audit/"invocation.json", dict(argv=raw, cwd=str(Path.cwd())))
    flags = list(expand(raw))
    require(flags.count("-o") == 1, "one output expected")
    oi = flags.index("-o")+1
    output = Path(flags[oi]).absolute()
    require(output.resolve() == output and output.parent == target/TARGET/"release/deps" and output.name.startswith("jnox_kernel-")
            and not output.is_symlink(), "unexpected kernel output")
    script_count = 0
    ei = None
    for i, arg in enumerate(flags):
        if i == oi or arg in ("-o", "gnu", "-flavor", "-T", "-L"):
            continue
        if (i and flags[i-1] == "-L") or arg.startswith("-L"):
            continue  # explicit libraries only; implicit -l lookups are rejected below
        script = arg.startswith("-T") or (i and flags[i-1] == "-T")
        if script:
            p = Path(arg[2:] if arg.startswith("-T") else arg).resolve(strict=True)
            require(p == ROOT/"kernel/native-extension.ld", "unexpected linker script")
            kind = "script"
            script_count += 1
        elif arg.startswith("-"):
            require(arg in ("--as-needed", "--no-as-needed", "-Bstatic", "-Bdynamic", "--gc-sections",
                            "--eh-frame-hdr", "--strip-debug", "--no-eh-frame-hdr", "--no-undefined",
                            "--start-group", "--end-group", "--whole-archive", "--no-whole-archive",
                            "--push-state", "--pop-state", "-static", "-O1", "-O2", "--build-id=none", "--fix-cortex-a53-843419",
                            "--no-rosegment", "--no-relax", "--discard-all", "-z")
                    or arg.startswith("--compress-debug-sections="), "unsupported linker flag: "+arg)
            continue
        elif i and flags[i-1] == "-z" and arg in ("noexecstack", "relro", "now"):
            continue
        else:
            p = Path(arg).resolve(strict=True)
            require(p.suffix in (".o", ".a", ".rlib"), "unknown linker input "+arg)
            kind = "object" if p.suffix == ".o" else "library"
            if p.name == "jnox_extension_entry.o":
                require(ei is None and p.is_relative_to(target) and p.parent.name == "out",
                        "unexpected or duplicate evaluator entry")
                ei = i
                kind = "entry"
        inputs.append(dict(index=i, argument=arg, path=str(p), sha256=retain(p), kind=kind))
    require(ei is not None and script_count == 1 and any(r["kind"] == "object" for r in inputs),
            "trusted object, fixed entry and single script required")
    tools = spec["tools"]
    for pin in tools.values():
        require(digest(Path(pin["path"])) == pin["sha256"], "tool digest changed")
    src = ROOT/"kernel/src/native_extension_entry.S"
    require(digest(src) == spec["entry_source_sha256"], "entry source changed")
    save(audit/"inputs.json", dict(inputs=inputs, expanded_flags=flags, responses=responses,
                                  raw_flags=raw, entry_index=ei, output_index=oi, tools=tools))
    for variant in ("ordinary", "dormant", "active"):
        args = flags.copy()
        entry = Path(flags[ei]).resolve()
        dest = output
        if variant != "ordinary":
            value = 1 if variant == "dormant" else 2 if spec["position"] == "a" else 3
            entry = audit/f"{variant}.o"
            arm = [tools["clang"]["path"], "--target=aarch64-none-elf", "-c", str(src),
                   f"-DJNOX_EXTENSION_STOP_VARIANT={value}", "-o", str(entry)]
            run_logged(arm, audit/f"asm-{variant}", cwd=ROOT/"kernel")
            args[ei] = str(entry)
            dest = audit/f"{variant}.elf"
            args[oi] = str(dest)
        for item in inputs:
            require(digest(Path(item["path"])) == item["sha256"], "trusted input changed between links")
        linkmap = audit/f"{variant}.map"
        argv = [tools["rust-lld"]["path"], *args, "-Map="+str(linkmap)]
        run_logged(argv, audit/f"link-{variant}", cwd=ROOT/"kernel")
        save(audit/f"{variant}.json", dict(argv=argv, exit=0, output_sha256=digest(dest),
                                          entry_sha256=retain(entry), map_sha256=digest(linkmap)))


def main():
    root = Path(os.environ["COHORT349_ROOT"])
    require(root.resolve(strict=True) == root and root.stat().st_uid == os.getuid()
            and root.stat().st_mode & 0o077 == 0, "private owned cohort required")
    spec = json.loads((root/"spec.json").read_text())
    require(spec["root"] == str(root) and spec["source"] == str(ROOT) and spec["position"] in ("a", "b")
            and digest(Path(__file__).resolve()) == spec["wrapper_sha256"], "cohort binding")
    audit = root/"link"
    audit.mkdir()  # fail closed on repeated target-link invocation
    try:
        link(root, spec, audit)
    except Exception as exc:
        save(audit/"failure.json", dict(type=type(exc).__name__, message=str(exc)))
        raise


if __name__ == "__main__":
    main()
