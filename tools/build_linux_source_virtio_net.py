#!/usr/bin/env python3
"""Compile Linux's complete pinned virtio_net.c and link its validate callback for JNOX."""
import argparse
import json
import os
import re
import shlex
import shutil
import struct
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


def select_poll_faults(source, variant, work, *, lifecycle=False):
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
    if lifecycle:
        for row in rows:
            if row["symbol"] == "netif_tx_wake_queue":
                row["native_provider"] = "lifecycle_netdev.o"
    selection = work / "jnox_rx_fault_select.inc"
    selection.write_text("".join(f".set jnox_enable_{r['id']}, 1\n" for r in rows
                                 if r["original_branch_sites"] and not r.get("native_provider")))
    return {"version": 1, "fault_symbol": "jnox_rx_poll_fault_id", "exit_status": 121,
            "source_sha256": sha(source.read_bytes()), "imports": rows}


def module_entry_provenance(original, promoted, linked, link_map):
    # Kbuild emits these public aliases in the special module init/exit
    # sections. Compare the actual symbol slices, even for a shared section.
    before, after = ObjectGraph(original), ObjectGraph(promoted)
    evidence = {}
    for name, local in (("init_module", "virtio_net_driver_init"),
                        ("cleanup_module", "virtio_net_driver_exit")):
        locations = []
        hashes = []
        for obj in (before, after):
            keys = obj.function_names[name]
            if len(keys) != 1:
                raise ValueError(f"missing original module entry {name}")
            key, = keys
            if obj.function_names[local] != [key]:
                raise ValueError(f"upstream module alias mismatch: {local}/{name}")
            idx, offset, size = key
            start = obj.sections[idx][4] + offset
            locations.append((obj.names[idx], offset, size))
            hashes.append(sha(obj.data[start:start + size]))
        section, offset, size = locations[0]
        if len(set(locations)) != 1 or len(set(hashes)) != 1:
            raise ValueError(f"module entry changed during export: {name}")
        suppliers = [line for line in link_map.splitlines()
                     if line.strip().endswith(promoted.name + ":(" + section + ")")]
        public = defined(linked, name, "GLOBAL")
        if len(suppliers) != 1 or len(public) != 1:
            raise ValueError(f"missing original linked module entry: {name}")
        if int(suppliers[0].split()[0], 16) + offset != int(public[0]["value"], 16):
            raise ValueError(f"incorrect ELF address for original module entry: {name}")
        evidence[name] = {"alias": local, "section": section, "offset": offset,
                          "size": size, "original_sha256": hashes[0], "promoted_sha256": hashes[1],
                          "linked_address": int(public[0]["value"], 16), "supplier": promoted.name}
    return evidence


def load_address_bytes(path, address, size):
    data = path.read_bytes()
    phoff = struct.unpack_from("<Q", data, 32)[0]
    stride, count = struct.unpack_from("<HH", data, 54)
    segments = [struct.unpack_from("<IIQQQQQQ", data, phoff + i * stride) for i in range(count)]
    ranges = [p for p in segments if p[0] == 1 and p[3] <= address and address + size <= p[3] + p[5]]
    if len(ranges) != 1:
        raise ValueError(f"unmapped ELF bytes at {address:#x}")
    offset = ranges[0][2] + address - ranges[0][3]
    return data[offset:offset + size]


def linked_ops_tables(path, elf, graph, original, derivative, link_map):
    """Bind the original table relocations to bytes from PT_LOAD in the linked ELF."""
    before, after = ObjectGraph(original), ObjectGraph(derivative)
    result = {}
    for table, funcs in {"virtio_net_driver": ("virtnet_validate", "virtnet_probe", "virtnet_remove"),
                         "virtnet_netdev": ("virtnet_open", "virtnet_close", "start_xmit")}.items():
        objects = [s for s in elf["symbols"] if s["name"] == table and s["type"] == "OBJECT" and s["index"].isdigit()]
        if len(objects) != 1:
            raise ValueError(f"missing unique linked ops table {table}")
        table_inputs = []
        for obj in (before, after):
            keys = [key for key, node in obj.nodes.items() if node["name"] == table and node["kind"] == "data"]
            if len(keys) != 1:
                raise ValueError(f"missing original table bytes: {table}")
            (idx, offset, size), = keys
            start = obj.sections[idx][4] + offset
            table_inputs.append((obj.names[idx], offset, size, sha(obj.data[start:start + size])))
        if table_inputs[0] != table_inputs[1] or objects[0]["size"] != table_inputs[0][2]:
            raise ValueError(f"upstream table changed during export/link: {table}")
        address = int(objects[0]["value"], 16)
        rows = []
        for name in funcs:
            refs = [r for r in graph["entries"][name]["entry_pointer_locations"] if r["table"] == table]
            if len(refs) != 1 or refs[0]["offset"] + 8 > objects[0]["size"]:
                raise ValueError(f"missing original ABS64 pointer {table} -> {name}")
            ref = refs[0]
            if sum(line.strip().endswith(derivative.name + ":(" + ref["section"] + ")")
                   for line in link_map.splitlines()) != 1:
                raise ValueError(f"unexpected linked ops table supplier: {table}")
            field = address + ref["offset"]
            target = int.from_bytes(load_address_bytes(path, field, 8), "little")
            syms = defined(elf, name, "GLOBAL")
            if len(syms) != 1 or int(syms[0]["value"], 16) != target:
                raise ValueError(f"invalid linked pointer {table} -> {name}")
            rows.append({**ref, "callback": name, "linked_pointer_address": field, "linked_target": target})
        result[table] = {"linked_address": address, "object_supplier": derivative.name,
                         "section": table_inputs[0][0], "size": table_inputs[0][2],
                         "original_sha256": table_inputs[0][3], "promoted_sha256": table_inputs[1][3],
                         "members": rows}
    return result


def build(args, work, profile, data):
    lifecycle_service = args.lifecycle_service or args.lifecycle_duplex_service
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
    irq_service = args.rx_irq_service or args.rx_restart_service or lifecycle_service
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
        if lifecycle_service:
            packet_kernel = ["-DJNOX_PACKET_LIFECYCLE_ABI=1", *packet_kernel]
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
            if lifecycle_service:
                roots.append("virtnet_close")
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
        if lifecycle_service:
            callbacks.extend(("virtnet_probe", "virtnet_remove", "virtnet_open", "virtnet_close", "start_xmit"))
        exports = callbacks + (["virtio_net_driver", "napi_tx"] if lifecycle_service else [])
        invoke(tools["objcopy"], *["--globalize-symbol=" + name for name in exports],
               callback_input.name, derivative.name)
        full = inspect_elf(callback_input, tools)
        exported = inspect_elf(derivative, tools)
        if lifecycle_service:
            for elf, binding in ((full, "LOCAL"), (exported, "GLOBAL")):
                if len([s for s in elf["symbols"] if s["type"] == "OBJECT" and s["bind"] == binding
                        and s["name"] == "virtio_net_driver" and s["index"].isdigit()]) != 1:
                    raise ValueError("expected a single genuine virtio_net_driver object export")
            for objfile, elf, binding in ((variant, full, "LOCAL"), (derivative, exported, "GLOBAL")):
                graph = ObjectGraph(objfile)
                keys = [k for k, node in graph.nodes.items() if node["kind"] == "data" and node["name"] == "napi_tx"]
                syms = [s for s in elf["symbols"] if s["name"] == "napi_tx" and s["type"] == "OBJECT"
                        and s["bind"] == binding and s["size"] == 1]
                if len(keys) != 1 or len(syms) != 1:
                    raise ValueError("missing unique Linux napi_tx parameter byte")
                (idx, offset, size), = keys
                if size != 1 or graph.data[graph.sections[idx][4] + offset] != 1:
                    raise ValueError("unexpected Linux napi_tx compiled default")
                napi_parameter = {"name": "napi_tx", "section": graph.names[idx], "offset": offset,
                                  "default": True, "requested_before_module_init": False}
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
            if args.lifecycle_duplex_service:
                runtime_kernel = [*runtime_kernel, "-DJNOX_PACKET_DUPLEX_ABI=1"]
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
                if lifecycle_service:
                    adapters = (*adapters, "lifecycle_bus", "lifecycle_netdev", "lifecycle_platform", "lifecycle_common")
                if args.lifecycle_duplex_service:
                    adapters = (*adapters, "transmit")
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
            if lifecycle_service:
                app = ROOT / "examples/linux-source-net-lifecycle/driver.c"
                result["mode"] = "lifecycle-service"
                result["queue_service_scope"] = "Linked upstream callbacks and ops tables; registration and execution require guest receipts."
            if args.lifecycle_duplex_service:
                app = ROOT / "examples/linux-source-net-duplex/driver.c"
                result["mode"] = "lifecycle-duplex-service"
                result["queue_service_scope"] = "One native lifecycle owner with upstream registered TX and RX callbacks on a shared queue pair."
            if args.rx_restart_service:
                fixture = ROOT / "examples/linux-source-net-restart"
                app = fixture / "primary.c"
            for source, output in [(app, "driver.o"), (SDK / "start.S", "start.o"),
                                   (SDK / "io.c", "io.o")]:
                app_flags = [*native, "-DJNOX_NATIVE_RX_POLL_FIXTURE=1"] if poll_service and source == app else native
                if irq_service and source == app:
                    app_flags = [*app_flags, "-DJNOX_NATIVE_RX_IRQ_FIXTURE=1"]
                    if lifecycle_service:
                        app_flags.append("-DJNOX_NATIVE_LIFECYCLE_FIXTURE=1")
                    if args.lifecycle_duplex_service:
                        app_flags.append("-DJNOX_NATIVE_LIFECYCLE_DUPLEX_FIXTURE=1")
                    if args.rx_restart_service:
                        app_flags.append("-DJNOX_NATIVE_RX_RESTART_FIXTURE=1")
                    if args.rx_irq_rearm_test:
                        app_flags.append("-DJNOX_NATIVE_RX_IRQ_RACE_FIXTURE=1")
                compile_tu(tools["clang"], app_flags, source, output)
            linkscript = SDK / "app.ld"
            files[linkscript.relative_to(ROOT).as_posix()] = sha(linkscript.read_bytes())
            if poll_service:
                guard = ROOT / "drivers/linux-compat/packet/rx_poll_unsupported.S"
                result["poll_fault_table"] = select_poll_faults(guard, variant, work, lifecycle=lifecycle_service)
                compile_tu(tools["clang"], [*native, "-I", work], guard, "rx_poll_unsupported.o")
                extra_objects.append("rx_poll_unsupported.o")
            if lifecycle_service:
                guard = ROOT / "drivers/linux-compat/packet/lifecycle_unsupported.S"
                compile_tu(tools["clang"], [*native, "-DJNOX_PACKET_LIFECYCLE_ABI=1", "-I", work],
                           guard, "lifecycle_unsupported.o")
                extra_objects.append("lifecycle_unsupported.o")
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
            forbidden_lifecycle = [] if lifecycle_service else ["init_module", "cleanup_module"]
            if not lifecycle_service:
                forbidden_lifecycle.extend(("virtnet_probe", "virtnet_remove"))
            for symbol in forbidden_lifecycle:
                if any(s["name"] == symbol for s in service["symbols"]):
                    raise ValueError(f"unexpected Linux lifecycle root in the service: {symbol}")
            if queue_mode:
                # IRQ discovery passes through the upstream registrar into the
                # same transport callback. Earlier profiles synthesize the pair.
                registrar = "virtnet_find_vqs" if irq_service else "jnox_net_setup_queues"
                for symbol in ("jnox_queue_find_vqs", registrar):
                    if not defined(service, symbol, "GLOBAL"):
                        raise ValueError(f"missing queue-service implementation: {symbol}")
                forbidden = [] if lifecycle_service else ["virtnet_open"]
                if irq_service:
                    forbidden.append("jnox_net_setup_queues")
                if not poll_service:
                    forbidden.append("virtnet_poll")
                if data_service:
                    if receive_service and not lifecycle_service:
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
                if lifecycle_service:
                    param_syms = [s for s in service["symbols"] if s["name"] == "napi_tx" and
                                  s["type"] == "OBJECT" and s["bind"] == "GLOBAL" and s["size"] == 1]
                    supplier = derivative.name + ":(" + napi_parameter["section"] + ")"
                    suppliers = [line for line in link_map.splitlines() if line.strip().endswith(supplier)]
                    if len(param_syms) != 1 or len(suppliers) != 1 or (
                            int(suppliers[0].split()[0], 16) + napi_parameter["offset"] != int(param_syms[0]["value"], 16)):
                        raise ValueError("wrong linked Linux napi_tx parameter supplier")
                    address = int(param_syms[0]["value"], 16)
                    if load_address_bytes(work / "service.elf", address, 1) != b"\x01":
                        raise ValueError("unexpected linked initial value for Linux napi_tx")
                    result["napi_tx_parameter"] = {**napi_parameter, "linked_vma": address}
                    result["module_entries"] = module_entry_provenance(variant, derivative, service, link_map)
                    result["lifecycle_ops"] = linked_ops_tables(work / "service.elf", service,
                        reports[variant.name]["relocations"], variant, derivative, link_map)
                    peer_source = ROOT / "examples/linux-source-net-lifecycle/peer.c"
                    peer_flags += ["-DJNOX_NATIVE_LIFECYCLE_FIXTURE=1", "-DJNOX_NATIVE_RX_POLL_FIXTURE=1"]
                    peer_symbol, peer_words = "jnox_lifecycle_peer", 24
                    if args.lifecycle_duplex_service:
                        member = next(row for row in result["lifecycle_ops"]["virtnet_netdev"]["members"]
                                      if row["callback"] == "start_xmit")
                        tx, = defined(service, "start_xmit", "GLOBAL")
                        upstream_tx, = defined(full, "start_xmit", "LOCAL")
                        if (member["relocation"] != "R_AARCH64_ABS64" or
                                int(tx["value"], 16) != member["linked_target"] or
                                not tx["size"] or tx["size"] != upstream_tx["size"]):
                            raise ValueError("duplex ndo_start_xmit lacks a linked original body")
                        result["duplex_tx"] = {"member": "ndo_start_xmit", "original_object": variant.name,
                            "linked_object": derivative.name, "adapter_object": "transmit.o",
                            "linked_entry": member["linked_target"], "linked_size": tx["size"],
                            "adapter_sha256": sha((work / "transmit.o").read_bytes()),
                            "table_offset": member["offset"]}
                        if not any(sym["name"] == "jnox_duplex_driver" and sym["type"] == "OBJECT" and
                                   sym["bind"] == "GLOBAL" and sym["size"] == 256 and sym["index"].isdigit()
                                   for sym in service["symbols"]):
                            raise ValueError("missing duplex driver receipt")
                        peer_source = ROOT / "examples/linux-source-net-duplex/peer.c"
                        peer_symbol, peer_words = "jnox_duplex_peer", 16
                        peer_flags.append("-DJNOX_NATIVE_LIFECYCLE_DUPLEX_FIXTURE=1")
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
                if lifecycle_service:
                    expected = {"jnox_lifecycle_peer": 192, "jnox_lifecycle_peer_frames": 80 * 91,
                                "jnox_lifecycle_peer_ack": 60, "jnox_lifecycle_peer_second": 120}
                    if args.lifecycle_duplex_service:
                        expected = {"jnox_duplex_peer": 128, "jnox_duplex_peer_requests": 80 * 91,
                                    "jnox_duplex_peer_replies": 80 * 91, "jnox_duplex_peer_ack": 60}
                    for name, size in expected.items():
                        if len([sym for sym in peer["symbols"] if sym["name"] == name and sym["type"] == "OBJECT"
                                and sym["bind"] == "GLOBAL" and sym["index"].isdigit() and sym["size"] == size]) != 1:
                            raise ValueError(f"lifecycle peer lacks expected evidence buffer: {name}")
                    result["lifecycle_peer_evidence_bytes"] = expected
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
    mode.add_argument("--lifecycle-duplex-service", action="store_true",
                      help="link one original lifecycle netdev with its registered RX and TX operations")
    mode.add_argument("--lifecycle-service", action="store_true",
                      help="link original virtio-net driver, netdev operations and bounded native bus/core")
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
