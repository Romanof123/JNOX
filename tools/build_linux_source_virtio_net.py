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
from linux_driver_relocations import ObjectGraph, inventory
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


def select_poll_faults(source, variant, work):
    # A NOTYPE undefined ELF symbol alone is insufficient evidence of a
    # function. Verify actual direct branch relocation and opcode at each site.
    obj = ObjectGraph(variant)
    imports = {}
    for section, rels in obj.relocations.items():
        sh = obj.sections[section]
        if not sh[2] & 4:  # SHF_EXECINSTR
            continue
        for off, kind, (name, symbol_type, idx, _, _), addend in rels:
            if idx or symbol_type not in (0, 2) or addend or kind not in (282, 283):
                continue
            word = int.from_bytes(obj.data[sh[4] + off:sh[4] + off + 4], "little")
            if word >> 26 != {282: 0b000101, 283: 0b100101}[kind]:
                continue
            imports.setdefault(name, []).append({"section": obj.names[section], "offset": off,
                "relocation": "R_AARCH64_CALL26" if kind == 283 else "R_AARCH64_JUMP26"})
    rows = []
    ids, names = set(), set()
    for line in source.read_text().splitlines():
        if not line.startswith("rx_unsupported "):
            continue
        match = re.fullmatch(r"rx_unsupported (\d+), (\w+), (\w+)", line)
        if not match:
            raise ValueError(f"invalid RX admission table row: {line}")
        number, name, reason = int(match[1]), match[2], match[3]
        if number in ids or name in names or not 0 < number <= 65535:
            raise ValueError(f"duplicate or invalid RX fault ID/name: {line}")
        ids.add(number)
        names.add(name)
        rows.append({"id": number, "symbol": name, "reason": reason,
                     "original_branch_sites": imports.get(name, [])})
    selection = work / "jnox_rx_fault_select.inc"
    selection.write_text("".join(f".set jnox_enable_{r['id']}, 1\n" for r in rows if r["original_branch_sites"]))
    return {"version": 1, "fault_symbol": "jnox_rx_poll_fault_id", "exit_status": 121,
            "source_sha256": sha(source.read_bytes()), "imports": rows}


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
    irq_service = args.rx_irq_service or args.rx_restart_service
    poll_service = args.rx_poll_service or irq_service
    receive_service = args.rx_service or poll_service
    data_service = args.packet_service or receive_service
    queue_mode = args.queue_service or data_service
    packet_mode = args.packet_object_only or queue_mode
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
        if data_service:
            packet_kernel = ["-DJNOX_PACKET_SERVICE=1", "-DJNOX_PACKET_TX_ABI=1", *packet_kernel]
        original_flags = packet_kernel
        if receive_service:
            packet_kernel = ["-DJNOX_PACKET_RX_ABI=1", *packet_kernel]
            if poll_service:
                packet_kernel = ["-DJNOX_PACKET_RX_POLL_ABI=1", *packet_kernel]
                if irq_service:
                    packet_kernel = ["-DJNOX_PACKET_RX_IRQ_ABI=1", *packet_kernel]
                    if args.rx_irq_rearm_test:
                        packet_kernel = ["-DJNOX_PACKET_RX_IRQ_RACE_ABI=1", *packet_kernel]
            # The driver normally inlines its private receive helpers. Preserve
            # separate definitions in the original TU; gc-sections discards
            # unreferenced functions at link time.
            from linux_driver_tx_layout import upstream_prelude
            rx_prefix = work / "jnox_rx_original_prefix.h"
            prefix_text = upstream_prelude(data.decode("utf-8")) + "\n#include <jnox_rx_page_api.h>\n"
            if irq_service:
                prefix_text += ("#undef sprintf\n"
                                "extern int jnox_rx_queue_name_v1(char *, const char *, unsigned int);\n"
                                "#define sprintf(dst,fmt,val) jnox_rx_queue_name_v1((dst),(fmt),(val))\n")
            rx_prefix.write_text(prefix_text, encoding="utf-8")
            original_flags = [*packet_kernel, "-fkeep-static-functions", "-include", rx_prefix]
        compile_tu(tools["cc"], original_flags, upstream.name, variant.name)
        upstream.unlink()
        entries = profile["inventory_entries"]
        reports = {}
        for obj in (original, variant):
            roots = list(entries)
            if receive_service and obj == variant:
                roots.extend(("add_recvbuf_small", "virtnet_rq_get_buf"))
                if poll_service:
                    roots.append("virtnet_alloc_queues")
                    if irq_service:
                        roots.extend(("virtnet_find_vqs", "skb_recv_done"))
            graph = inventory(obj, tools["objdump"], roots)
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
        callback_input = variant if queue_mode else original
        callbacks = [callback] + (["start_xmit"] if args.packet_service else [])
        if receive_service:
            callbacks.extend(("add_recvbuf_small", "virtnet_rq_get_buf"))
            if poll_service:
                callbacks.extend(("virtnet_alloc_queues", "virtnet_poll"))
                if irq_service:
                    callbacks.extend(("virtnet_find_vqs", "skb_recv_done"))
        invoke(tools["objcopy"], *["--globalize-symbol=" + name for name in callbacks],
               callback_input.name, derivative.name)
        full = inspect_elf(callback_input, tools)
        exported = inspect_elf(derivative, tools)
        checked_callbacks = {}
        for name in callbacks:
            section = ".text." + name
            local = defined(full, name, "LOCAL")
            public = defined(exported, name, "GLOBAL")
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
            checked_callbacks[name] = {"name": name, "section": section, "size": len(payloads[0]),
                                       "original_section_sha256": sha(payloads[0]),
                                       "promoted_section_sha256": sha(payloads[1])}
        control_report = {
            "schema": 1, "profile": profile["name"], "mode": "object-only" if args.object_only else "service",
            "upstream": profile["upstream"], "header_profile": profile["headers"],
            "source": {"path": str(args.source.resolve()), "size": len(data), "sha256": sha(data)},
            "header_root": str(headers), "header_tree_sha256": header_tree_sha256,
            "driver_object": full, "callback_object": exported,
            "lifecycle_relocations": (reports[variant.name]["relocations"] if queue_mode else
                                      inventory(original, tools["objdump"], profile["inventory_entries"])),
            "callback": checked_callbacks[callback],
        }
        if queue_mode:
            result.update(control_report)
            result["mode"] = "packet-service" if args.packet_service else "queue-service"
            if receive_service:
                result["mode"] = "rx-poll-service" if poll_service else "rx-service"
                if irq_service:
                    result["mode"] = "rx-restart-service" if args.rx_restart_service else "rx-irq-service"
                    graph = reports[variant.name]["relocations"]
                    registrar = graph["entries"]["virtnet_find_vqs"]["root"]
                    callback_id = graph["entries"]["skb_recv_done"]["root"]
                    refs = [ref for ref in graph["nodes"][registrar]["references"]
                            if ref["kind"] == "address" and ref["to"] == callback_id
                            and ref.get("target_offset") == 0]
                    if not refs:
                        raise ValueError("missing original RX registration address relocation")
                    result["irq_registration"] = {
                        "source_object": derivative.name, "registrar": "virtnet_find_vqs",
                        "queue_index": 0, "callback": "skb_recv_done", "object_address_references": refs,
                        "scope": "Actual queue registration and IRQ delivery require guest evidence.",
                    }
            result["callback_input"] = variant.name
            result["queue_service_scope"] = "Source validation and native bus queue setup only; Linux TX, RX and lifecycle entry points remain unadmitted."
        else:
            result = control_report
        if not args.object_only:
            adapter = ROOT / profile["adapter_source"]
            runtime_kernel = packet_kernel + ["-DJNOX_QUEUE_SERVICE=1"] if queue_mode else kernel
            compile_tu(tools["cc"], runtime_kernel, adapter, "adapter.o")
            extra_objects = []
            app = ROOT / profile["service_source"]
            if queue_mode:
                app = ROOT / "examples/linux-source-net-queues/driver.c"
                for source, output in [("drivers/linux-compat/src/virtio_net_queues.c", "queues.o"),
                                       ("drivers/linux-compat/packet/context.c", "context.o")]:
                    compile_tu(tools["cc"], runtime_kernel, ROOT / source, output)
                    extra_objects.append(output)
            if data_service:
                from linux_driver_tx_layout import private_layout
                app = ROOT / "examples/linux-source-net-packets/driver.c"
                if receive_service:
                    app = ROOT / "examples/linux-source-net-rx/driver.c"
                generated_header = work / "jnox_virtnet_layout.h"
                generated_header.write_text(private_layout(data.decode("utf-8"), receive=receive_service), encoding="utf-8")
                result["layout_view"] = {"file": generated_header.name, "sha256": sha(generated_header.read_bytes()),
                    "generator_sha256": sha((ROOT / "tools/linux_driver_tx_layout.py").read_bytes())}
                result["callbacks"] = checked_callbacks
                result["queue_service_scope"] = "Original validation and TX callback plus native transport. Full Linux lifecycle and RX poll remain unadmitted."
                if receive_service:
                    result["queue_service_scope"] = "Original RX small-buffer provisioning and used-buffer retrieval. Full Linux RX poll and lifecycle remain unadmitted."
                    receive_adapters = ("rx_poll", "rx_napi", "rx_skb") if poll_service else ("receive",)
                    adapters = (*receive_adapters, "rx_pages", "rx_virtqueue", "tx_virtqueue", "tx_runtime")
                    if poll_service:
                        result["queue_service_scope"] = "Upstream queue allocation and RX poll with the bounded native packet adapters; guest execution requires separate evidence."
                        result["retention_only"] = ["virtnet_rq_get_buf"]
                else:
                    adapters = ("transmit", "tx_virtqueue", "tx_runtime")
                for name in adapters:
                    source = ROOT / ("drivers/linux-compat/packet/" + name + ".c")
                    compile_tu(tools["cc"], [*runtime_kernel, "-I", work], source, name + ".o")
                    extra_objects.append(name + ".o")
            native = ["--target=aarch64-none-elf", *COMMON, "-I", SDK / "include"]
            if queue_mode:
                compile_tu(tools["clang"], [*native, "-fno-builtin"], SDK / "memset.c", "memset.o")
                extra_objects.append("memset.o")
            if data_service:
                compile_tu(tools["clang"], [*native, "-fno-builtin"], SDK / "memcpy.c", "memcpy.o")
                extra_objects.append("memcpy.o")
            if args.rx_restart_service:
                fixture = ROOT / "examples/linux-source-net-restart"
                app = fixture / "primary.c"
            for source, output in [(app, "driver.o"), (SDK / "start.S", "start.o"),
                                   (SDK / "io.c", "io.o")]:
                app_flags = [*native, "-DJNOX_NATIVE_RX_POLL_FIXTURE=1"] if poll_service and source == app else native
                if irq_service and source == app:
                    app_flags = [*app_flags, "-DJNOX_NATIVE_RX_IRQ_FIXTURE=1"]
                    if args.rx_restart_service:
                        app_flags.append("-DJNOX_NATIVE_RX_RESTART_FIXTURE=1")
                    if args.rx_irq_rearm_test:
                        app_flags.append("-DJNOX_NATIVE_RX_IRQ_RACE_FIXTURE=1")
                compile_tu(tools["clang"], app_flags, source, output)
            linkscript = SDK / "app.ld"
            files[linkscript.relative_to(ROOT).as_posix()] = sha(linkscript.read_bytes())
            if poll_service:
                guard = ROOT / "drivers/linux-compat/packet/rx_poll_unsupported.S"
                result["poll_fault_table"] = select_poll_faults(guard, variant, work)
                compile_tu(tools["clang"], [*native, "-I", work], guard, "rx_poll_unsupported.o")
                extra_objects.append("rx_poll_unsupported.o")
            # Inlining inside the original poll may eliminate the reference to
            # this standalone helper. Retention does not establish execution.
            retain = ["--error-limit=0", "--undefined=virtnet_rq_get_buf", "--undefined=jnox_rx_poll_fault_id"] if poll_service else []
            invoke(tools["ld"], *retain, "-m", "aarch64elf", "-static", "--no-undefined", "--gc-sections",
                   "--build-id=none", "-z", "max-page-size=4096", "-T", linkscript, "-Map=service.map",
                   "start.o", "io.o", "driver.o", "adapter.o", *extra_objects, derivative.name, "-o", "service.elf")
            service = inspect_elf(work / "service.elf", tools)
            if irq_service:
                witness = [sym for sym in service["symbols"] if sym["name"] == "jnox_irq_race_gate"
                           and sym["index"].isdigit()]
                if len(witness) != int(args.rx_irq_rearm_test):
                    raise ValueError("IRQ race witness conflicts with the selected service profile")
                result["irq_rearm_test"] = {"test_only": args.rx_irq_rearm_test,
                    "witness": witness[0] if witness else None}
            if (not defined(service, callback, "GLOBAL") or service["undefined"] or
                not re.search(r"Type:\s+EXEC\b", service["header"]) or
                re.search(r"^\s*(?:INTERP|DYNAMIC)\s", service["program_headers"], re.M) or
                ".note.jnox.abi" not in service["sections"] or not defined(service, "_start", "GLOBAL")):
                raise ValueError("static JNOX executable failed callback/ABI verification")
            for symbol in ("virtnet_probe", "virtnet_remove", "init_module", "cleanup_module"):
                if any(s["name"] == symbol for s in service["symbols"]):
                    raise ValueError(f"unexpected Linux lifecycle root in the service: {symbol}")
            if queue_mode:
                # IRQ discovery passes through the upstream registrar into the
                # same transport callback. Earlier profiles synthesize the pair.
                registrar = "virtnet_find_vqs" if irq_service else "jnox_net_setup_queues"
                for symbol in ("jnox_queue_find_vqs", registrar):
                    if not defined(service, symbol, "GLOBAL"):
                        raise ValueError(f"missing queue-service implementation: {symbol}")
                forbidden = ["virtnet_open"]
                if irq_service:
                    forbidden.append("jnox_net_setup_queues")
                if not poll_service:
                    forbidden.append("virtnet_poll")
                if data_service:
                    if receive_service:
                        forbidden.append("start_xmit")
                    link_map = (work / "service.map").read_text()
                    for name in callbacks:
                        if len([s for s in service["symbols"] if s["name"] == name]) != 1 or not defined(service, name, "GLOBAL"):
                            raise ValueError(f"expected one original linked callback: {name}")
                        supplier = derivative.name + ":(.text." + name + ")"
                        if sum(line.strip().endswith(supplier) for line in link_map.splitlines()) != 1:
                            raise ValueError(f"unexpected callback link-map provenance: {name}")
                else:
                    forbidden.append("start_xmit")
                for symbol in forbidden:
                    if any(s["name"] == symbol for s in service["symbols"]):
                        raise ValueError(f"unadmitted packet entry point in queue service: {symbol}")
            if irq_service:
                peer_source = ROOT / "examples/linux-source-net-rx/peer.c"
                peer_flags = [*native, "-DJNOX_NATIVE_RX_IRQ_FIXTURE=1"]
                peer_symbol, peer_words = "jnox_rx_irq_peer", 16
                if args.rx_restart_service:
                    peer_source = fixture / "peer.c"
                    peer_symbol, peer_words = "jnox_restart_peer", 32
                    peer_flags += ["-DJNOX_NATIVE_RX_RESTART_FIXTURE=1", "-DJNOX_NATIVE_RX_POLL_FIXTURE=1"]
                if args.rx_irq_rearm_test:
                    peer_flags.append("-DJNOX_NATIVE_RX_IRQ_RACE_FIXTURE=1")
                compile_tu(tools["clang"], peer_flags, peer_source, "irq_peer.o")
                invoke(tools["ld"], "-m", "aarch64elf", "-static", "--no-undefined", "--gc-sections",
                       "--build-id=none", "-z", "max-page-size=4096", "-T", linkscript, "-Map=peer.map",
                       "start.o", "io.o", "irq_peer.o", "-o", "peer.elf")
                peer = inspect_elf(work / "peer.elf", tools)
                peer_witness = [sym for sym in peer["symbols"] if sym["name"] == peer_symbol
                                and sym["type"] == "OBJECT" and sym["bind"] == "GLOBAL"
                                and sym["index"].isdigit() and int(sym["size"]) == peer_words * 8]
                if (len(peer_witness) != 1 or peer["undefined"] or not defined(peer, "_start", "GLOBAL")
                        or ".note.jnox.abi" not in peer["sections"]):
                    raise ValueError("IRQ peer lacks its native startup, ABI or expected timing witness")
                result["irq_peer"] = {"elf": peer, "slot": 1, "source": peer_source.relative_to(ROOT).as_posix(),
                    "witness": peer_witness[0], "test_only_rearm": args.rx_irq_rearm_test,
                    "link_map_sha256": sha((work / "peer.map").read_bytes())}
            if args.rx_restart_service:
                compile_tu(tools["clang"], peer_flags, fixture / "spare.c", "spare.o")
                invoke(tools["ld"], *retain, "-m", "aarch64elf", "-static", "--no-undefined", "--gc-sections",
                       "--build-id=none", "-z", "max-page-size=4096", "-T", linkscript, "-Map=spare.map",
                       "start.o", "io.o", "spare.o", "adapter.o", *extra_objects, derivative.name, "-o", "spare.elf")
                spare = inspect_elf(work / "spare.elf", tools)
                spare_map = (work / "spare.map").read_text()
                if (spare["undefined"] or not defined(spare, "_start", "GLOBAL")
                        or ".note.jnox.abi" not in spare["sections"]
                        or not re.search(r"Type:\s+EXEC\b", spare["header"])
                        or re.search(r"^\s*(?:INTERP|DYNAMIC)\s", spare["program_headers"], re.M)):
                    raise ValueError("spare lacks a statically linked native executable")
                for name in callbacks:
                    supplier = derivative.name + ":(.text." + name + ")"
                    if (len(defined(spare, name, "GLOBAL")) != 1
                            or sum(line.strip().endswith(supplier) for line in spare_map.splitlines()) != 1):
                        raise ValueError(f"spare lacks original callback provenance: {name}")
                for elf in (service, spare):
                    if not any(sym["name"] == "jnox_restart_driver" and sym["type"] == "OBJECT"
                               and sym["index"].isdigit() and sym["size"] == 256 for sym in elf["symbols"]):
                        raise ValueError("missing restart driver witness")
                if any(sym["name"] in {*forbidden, "jnox_irq_race_gate", "virtnet_probe", "virtnet_remove",
                                      "init_module", "cleanup_module"} for sym in spare["symbols"]):
                    raise ValueError("unadmitted lifecycle or race symbol in spare")
                result["restart_roles"] = [
                    {"slot": slot, "role": role, "source": (fixture / source).relative_to(ROOT).as_posix(),
                     "elf": report["filename"], "sha256": report["sha256"], "map": mapname,
                     "map_sha256": sha((work / mapname).read_bytes())}
                    for slot, role, source, report, mapname in
                    ((0, "primary", "primary.c", service, "service.map"),
                     (1, "survivor", "peer.c", peer, "peer.map"), (2, "spare", "spare.c", spare, "spare.map"))]
                result["spare"] = spare
            if poll_service:
                for row in result["poll_fault_table"]["imports"]:
                    name = row["symbol"]
                    supplier = f"rx_poll_unsupported.o:(.text.{name})"
                    row["linked_handler"] = any(line.strip().endswith(supplier) for line in link_map.splitlines())
                    if row["linked_handler"] and (not row["original_branch_sites"] or not defined(service, name, "GLOBAL")):
                        raise ValueError(f"unsupported RX handler lacks original callsite: {name}")
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
    parser.add_argument("--rx-irq-rearm-test", action="store_true",
                        help="enable the test-only RX IRQ rearm fixture; requires --rx-irq-service")
    mode.add_argument("--rx-restart-service", action="store_true",
                      help="link separate primary, survivor and replacement binaries for native RX IRQ recovery")
    mode.add_argument("--rx-irq-service", action="store_true",
                      help="link the original queue registration and RX callback for native GIC dispatch")
    mode.add_argument("--rx-poll-service", action="store_true",
                      help="link original virtio-net queue allocation and RX poll for native service")
    mode.add_argument("--rx-service", action="store_true",
                      help="link upstream small-buffer RX provisioning and completion with native transport")
    mode.add_argument("--packet-service", action="store_true",
                      help="link upstream validation and TX callbacks with the native packet transport")
    mode.add_argument("--queue-service", action="store_true",
                      help="link upstream validation and native queue setup with the packet context")
    mode.add_argument("--packet-object-only", action="store_true",
                      help="compare stock and packet-header objects without linking a service")
    for name, default in [("cc", "aarch64-linux-gnu-gcc"), ("objcopy", "aarch64-linux-gnu-objcopy"),
                          ("objdump", "aarch64-linux-gnu-objdump"),
                          ("clang", "clang"), ("ld", "ld.lld"), ("readelf", "readelf"), ("nm", "nm")]:
        parser.add_argument("--" + name, default=default)
    args = parser.parse_args()
    if args.rx_irq_rearm_test and not args.rx_irq_service:
        parser.error("--rx-irq-rearm-test requires --rx-irq-service")
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
