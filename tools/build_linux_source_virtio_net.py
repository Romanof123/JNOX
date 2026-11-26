#!/usr/bin/env python3
"""Compile Linux's complete pinned virtio_net.c and link its validate callback for JNOX."""
import argparse
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from import_linux_driver import ROOT, SDK, inspect_elf, pinned_source, run, sha
from linux_driver_relocations import inventory
from prepare_linux_virtio_net_headers import verify_header_tree

PROFILE = ROOT / "drivers/linux-compat/profiles/virtio-net.json"
COMMON = ["-O2", "-std=gnu11", "-ffreestanding", "-nostdinc", "-ffunction-sections",
          "-fdata-sections", "-fno-stack-protector", "-fno-asynchronous-unwind-tables",
          "-fno-unwind-tables", "-fno-pic", "-fno-pie", "-fno-lto", "-mgeneral-regs-only",
          "-mno-outline-atomics", "-mbranch-protection=none"]


def kernel_flags(headers, cc, work):
    # Include ordering follows Kbuild's LINUXINCLUDE / USERINCLUDE for an in-tree ARM64 build.
    dirs = ["arch/arm64/include", "arch/arm64/include/generated", "include",
            "arch/arm64/include/uapi", "arch/arm64/include/generated/uapi",
            "include/uapi", "include/generated/uapi"]
    include_flags = [flag for path in dirs for flag in ("-I", headers / path)]
    private = run([cc, "-print-file-name=include"], work).strip()
    return [*COMMON, "-fno-strict-aliasing", "-fno-common", "-fno-delete-null-pointer-checks",
            "-fno-ipa-icf", "-fno-reorder-blocks-and-partition", "-fno-strict-overflow",
            "-fshort-wchar", "-Werror=implicit-function-declaration", "-march=armv8-a",
            "-Wa,-march=armv8.5-a", '-DARM64_ASM_ARCH="armv8.5-a"', "-D__KERNEL__", "-DMODULE",
            '-DKBUILD_BASENAME="virtio_net"', '-DKBUILD_MODNAME="virtio_net"',
            "-D__KBUILD_MODNAME=kmod_virtio_net", '-DKBUILD_MODFILE="drivers/net/virtio_net"',
            *include_flags, "-isystem", private,
            "-include", headers / "include/linux/compiler-version.h",
            "-include", headers / "include/linux/kconfig.h",
            "-include", headers / "include/linux/compiler_types.h"]


def defined(report, name, bind):
    return [s for s in report["symbols"] if s["name"] == name and s["type"] == "FUNC"
            and s["index"].isdigit() and s["bind"] == bind]


def dependencies(depfile, work):
    text = depfile.read_text().replace("\\\n", " ")
    return sorted({(work / name).resolve() for name in shlex.split(text.split(": ", 1)[1])})


def build(args, work, profile, data):
    names = ["cc", "objcopy", "objdump", "readelf", "nm"] + ([] if args.object_only or args.packet_object_only else ["clang", "ld"])
    tools = {name: shutil.which(getattr(args, name)) for name in names}
    for name, path in tools.items():
        if path is None:
            raise ValueError(f"required tool missing: {getattr(args, name)}")
    commands = []

    def invoke(*argv):
        commands.append(list(map(str, argv)))
        return run(argv, work)

    headers = args.headers.resolve()
    header_tree_sha256 = verify_header_tree(headers)
    for path, expected in profile["headers"]["generated_sha256"].items():
        if sha((headers / path).read_bytes()) != expected:
            raise ValueError(f"header package/configuration mismatch: {path}")
    files = {}
    upstream = work / "virtio_net.c"
    upstream.write_bytes(data)
    kernel = kernel_flags(headers, tools["cc"], work)
    packet_mode = args.packet_object_only or args.queue_service
    diagnostics = {}
    packet_inputs = {}

    def compile_tu(cc, flags, source, output):
        dep = work / (output + ".d")
        command = list(map(str, [cc, *flags, "-MD", "-MF", dep.name, "-c", source, "-o", output]))
        commands.append(command)
        compiled = subprocess.run(command, cwd=work, stderr=subprocess.PIPE, text=True,
                                  env={**os.environ, "LC_ALL": "C"})
        sys.stderr.write(compiled.stderr)
        compiled.check_returncode()
        diagnostics[output] = sorted({line.split("warning:", 1)[1].strip()
                                      for line in compiled.stderr.splitlines() if "warning:" in line})
        inputs = {}
        for path in dependencies(dep, work):
            if path == upstream:
                key = profile["upstream"]["path"]
            elif path.is_relative_to(headers):
                key = "headers/" + path.relative_to(headers).as_posix()
            elif path.is_relative_to(ROOT):
                key = path.relative_to(ROOT).as_posix()
            else:
                key = str(path)
            files[key] = inputs[key] = sha(path.read_bytes())
        if packet_mode:
            packet_inputs[output] = inputs
        dep.unlink()

    original = work / "virtio_net.original.o"
    derivative = work / "virtio_net.callback.o"
    compile_tu(tools["cc"], kernel, upstream.name, original.name)
    if packet_mode:
        from linux_driver_el0_audit import inspect_closures
        overlay = ROOT / "drivers/linux-compat/packet/include"
        if not overlay.is_dir():
            raise ValueError("missing packet compatibility include tree")
        variant = work / "virtio_net.packet.o"
        packet_kernel = ["-I", overlay, "-DJNOX_PACKET_CONTEXT_ABI=1", *kernel]
        compile_tu(tools["cc"], packet_kernel, upstream.name, variant.name)
        upstream.unlink()
        entries = profile["inventory_entries"]
        reports = {}
        for obj in (original, variant):
            graph = inventory(obj, tools["objdump"], entries)
            reports[obj.name] = {
                "elf": inspect_elf(obj, tools), "relocations": graph,
                "direct_system_instructions": inspect_closures(obj, tools["objdump"], graph),
                "sha256": sha(obj.read_bytes()),
            }
        comparison = {}
        for entry in entries:
            stock = reports[original.name]["direct_system_instructions"][entry]["counts"]
            packet = reports[variant.name]["direct_system_instructions"][entry]["counts"]
            comparison[entry] = {k: {"stock": stock.get(k, 0), "packet": packet.get(k, 0)}
                                 for k in sorted(stock.keys() | packet.keys())}
        result = {
            "schema": 1, "mode": "packet-object-only", "profile": profile["name"],
            "upstream": profile["upstream"], "source": {"path": str(args.source.resolve()),
                "size": len(data), "sha256": sha(data)},
            "header_profile": profile["headers"], "header_root": str(headers),
            "header_tree_sha256": header_tree_sha256, "packet_overlay": str(overlay),
            "objects": reports, "direct_instruction_comparison": comparison,
            "input_sha256_by_object": packet_inputs,
            "queue_packet_runnable": False,
            "audit_scope": "Static direct-branch closure only. Counts include categorized system "
                "instructions and unknown encodings; instruction classification is not a complete "
                "ISA admission check. Imports, indirect targets, linked runtime and device execution "
                "require independent verification.",
        }
    else:
        upstream.unlink()
    callback = profile["callback"]
    if not args.packet_object_only:
        callback_input = variant if args.queue_service else original
        section = ".text." + callback
        invoke(tools["objcopy"], "--globalize-symbol=" + callback, callback_input.name, derivative.name)
        full = inspect_elf(callback_input, tools)
        exported = inspect_elf(derivative, tools)
        local = defined(full, callback, "LOCAL")
        public = defined(exported, callback, "GLOBAL")
        if len(local) != 1 or len(public) != 1 or local[0]["size"] != public[0]["size"]:
            raise ValueError("expected a single local callback promoted to a global function")
        payloads = []
        for file, report, sym in [(callback_input, full, local[0]), (derivative, exported, public[0])]:
            if not re.search(r"Machine:\s+AArch64", report["header"]):
                raise ValueError("driver object is not AArch64")
            if not re.search(r"\[\s*" + sym["index"] + r"\]\s+" + re.escape(section) + r"\s+PROGBITS\b",
                             report["sections"]):
                raise ValueError("callback lacks its own executable input section")
            # Provide disposable output to objcopy; dumping a section must never rewrite its input.
            invoke(tools["objcopy"], "--dump-section", section + "=callback.tmp", file.name, "dump.tmp.o")
            payloads.append((work / "callback.tmp").read_bytes())
            (work / "dump.tmp.o").unlink()
            (work / "callback.tmp").unlink()
        if payloads[0] != payloads[1] or not payloads[0] or full["undefined"] != exported["undefined"]:
            raise ValueError("callback instructions or whole-object imports changed during promotion")
        if int(local[0]["value"], 16) != 0 or len(payloads[0]) != local[0]["size"]:
            raise ValueError("callback symbol does not span its complete function section")
        control_report = {
            "schema": 1, "profile": profile["name"], "mode": "object-only" if args.object_only else "service",
            "upstream": profile["upstream"], "header_profile": profile["headers"],
            "source": {"path": str(args.source.resolve()), "size": len(data), "sha256": sha(data)},
            "header_root": str(headers), "header_tree_sha256": header_tree_sha256,
            "driver_object": full, "callback_object": exported,
            "lifecycle_relocations": (reports[variant.name]["relocations"] if args.queue_service else
                                      inventory(original, tools["objdump"], profile["inventory_entries"])),
            "callback": {"name": callback, "section": section, "size": len(payloads[0]),
                         "original_section_sha256": sha(payloads[0]), "promoted_section_sha256": sha(payloads[1])},
        }
        if args.queue_service:
            result.update(control_report)
            result["mode"] = "queue-service"
            result["callback_input"] = variant.name
            result["queue_service_scope"] = "Source validation and native bus queue setup only; Linux TX, RX and lifecycle entry points remain unadmitted."
        else:
            result = control_report
        if not args.object_only:
            adapter = ROOT / profile["adapter_source"]
            runtime_kernel = packet_kernel + ["-DJNOX_QUEUE_SERVICE=1"] if args.queue_service else kernel
            compile_tu(tools["cc"], runtime_kernel, adapter, "adapter.o")
            extra_objects = []
            app = ROOT / profile["service_source"]
            if args.queue_service:
                app = ROOT / "examples/linux-source-net-queues/driver.c"
                for source, output in [("drivers/linux-compat/src/virtio_net_queues.c", "queues.o"),
                                       ("drivers/linux-compat/packet/context.c", "context.o")]:
                    compile_tu(tools["cc"], runtime_kernel, ROOT / source, output)
                    extra_objects.append(output)
            native = ["--target=aarch64-none-elf", *COMMON, "-I", SDK / "include"]
            if args.queue_service:
                compile_tu(tools["clang"], [*native, "-fno-builtin"], SDK / "memset.c", "memset.o")
                extra_objects.append("memset.o")
            for source, output in [(app, "driver.o"), (SDK / "start.S", "start.o"),
                                   (SDK / "io.c", "io.o")]:
                compile_tu(tools["clang"], native, source, output)
            linkscript = SDK / "app.ld"
            files[linkscript.relative_to(ROOT).as_posix()] = sha(linkscript.read_bytes())
            invoke(tools["ld"], "-m", "aarch64elf", "-static", "--no-undefined", "--gc-sections",
                   "--build-id=none", "-z", "max-page-size=4096", "-T", linkscript, "-Map=service.map",
                   "start.o", "io.o", "driver.o", "adapter.o", *extra_objects, derivative.name, "-o", "service.elf")
            service = inspect_elf(work / "service.elf", tools)
            if (not defined(service, callback, "GLOBAL") or service["undefined"] or
                not re.search(r"Type:\s+EXEC\b", service["header"]) or
                re.search(r"^\s*(?:INTERP|DYNAMIC)\s", service["program_headers"], re.M) or
                ".note.jnox.abi" not in service["sections"] or not defined(service, "_start", "GLOBAL")):
                raise ValueError("static JNOX executable failed callback/ABI verification")
            for symbol in ("virtnet_probe", "virtnet_remove", "init_module", "cleanup_module"):
                if any(s["name"] == symbol for s in service["symbols"]):
                    raise ValueError(f"unexpected Linux lifecycle root in the service: {symbol}")
            if args.queue_service:
                for symbol in ("jnox_queue_find_vqs", "jnox_net_setup_queues"):
                    if not defined(service, symbol, "GLOBAL"):
                        raise ValueError(f"missing queue-service implementation: {symbol}")
                for symbol in ("virtnet_open", "start_xmit", "virtnet_poll"):
                    if any(s["name"] == symbol for s in service["symbols"]):
                        raise ValueError(f"unadmitted packet entry point in queue service: {symbol}")
            result["service"] = service
            result["link_map_sha256"] = sha((work / "service.map").read_bytes())
    config = (headers / "include/generated/autoconf.h").read_text()
    header_compiler = re.search(r'^#define CONFIG_CC_VERSION_TEXT "(.*)"$', config, re.M)
    caps = subprocess.check_output([tools["cc"], "-E", "-P", "-x", "c", "-"], cwd=work,
                                   input="__has_attribute(counted_by)\n", text=True).strip()
    result["compiler_compatibility"] = {
        "executable": tools["cc"], "gcc_full_version": run([tools["cc"], "-dumpfullversion"], work).strip(),
        "header_config_compiler": header_compiler[1] if header_compiler else None,
        "has_attribute_counted_by": int(caps) != 0,
        "warnings_by_translation_unit": diagnostics,
        "counted_by_limitation": "This build does not establish runtime bounds checking. "
                                 "An unsupported or ignored counted_by attribute supplies no compiler enforcement.",
    }
    result.update(commands=commands, input_sha256=files, profile_sha256=sha(PROFILE.read_bytes()),
                  tools={name: {"path": path, "version": run([path, "--version"], work).splitlines()[0]}
                         for name, path in tools.items()})
    (work / "import.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source", "headers", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--object-only", action="store_true")
    mode.add_argument("--queue-service", action="store_true",
                      help="link upstream validation and native queue setup with the packet context")
    mode.add_argument("--packet-object-only", action="store_true",
                      help="compare stock and packet-header objects without linking a service")
    for name, default in [("cc", "aarch64-linux-gnu-gcc"), ("objcopy", "aarch64-linux-gnu-objcopy"),
                          ("objdump", "aarch64-linux-gnu-objdump"),
                          ("clang", "clang"), ("ld", "ld.lld"), ("readelf", "readelf"), ("nm", "nm")]:
        parser.add_argument("--" + name, default=default)
    args = parser.parse_args()
    profile = json.loads(PROFILE.read_text(encoding="utf-8"))
    data = pinned_source(args.source, profile["upstream"])
    out = args.out.resolve()
    if out.exists():
        parser.error("--out must name a new directory")
    out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=out.parent) as stage:
        work = Path(stage) / "build"
        work.mkdir()
        build(args, work, profile, data)
        work.rename(out)
    print(out / "import.json")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as exc:
        raise SystemExit(str(exc)) from exc
