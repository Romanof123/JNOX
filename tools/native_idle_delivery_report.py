#!/usr/bin/env python3
"""Offline #339 cold-pair report. Paths are capture directories; --markers is the raw plugin file.

Example: python3 tools/native_idle_delivery_report.py --control cold01 --observed cold02
 --preflight setup/preflight.json --kernel packages/wake/kernel.elf --markers cold02/markers.bin
 --apps reporter.elf histogram.elf energy.elf

Supply --wait-calibration with the raw blocked/runnable child-trial receipt from the
same boot, UID and allowed CPUs. Nominal 16 ns/tick projections use adjacent, unaffected
observations as held-out diagnostics. No independent across-idle continuity/error
bound is available from these captures. Only causal host intervals enter attribution.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import sys

from native_idle_kernel_layout import elf_layout, nm_layout
from native_idle_markers import APP_HASHES, ELF_HASH, correlate, sites
from ordinary_amc_capture import FLAGS
from read_hierarchy_health import read_uart
from elftools.elf.elffile import ELFFile

QEMU = "0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8"
P = 15_625_000


def check(value, reason):
    if not value:
        raise ValueError(reason)


def load(path):
    return json.loads(path.read_text())


def digest(path):
    with path.open("rb") as stream:
        h = hashlib.sha256()
        for block in iter(lambda:stream.read(1024*1024), b""):
            h.update(block)
        return h.hexdigest()


def product_data(root, apps, linked):
    captured = nm_layout((root / "symbols").read_text())
    check(captured == linked, "captured kernel layout differs from pinned ELF")
    base, spaces = linked["_start"], linked["spaces"]
    check((root/"ram.bin").stat().st_size == linked["__stack_top"]-base,
          "terminal RAM dump differs from linked capture size")
    specs = [{"private_progress":"I", "private_result":"I", "report_deadline_ns":"3Q", "report_observed_ns":"3Q"},
             {"private_progress":"I", "private_result":"9I"}, {"private_progress":"I", "private_result":"Q"}]
    products = []
    with (root / "ram.bin").open("rb") as ram:
        for slot, (app, spec) in enumerate(zip(apps, specs)):
            check(digest(app) == APP_HASHES[slot], f"wrong application {slot}")
            with app.open("rb") as binary:
                symbols = ELFFile(binary).get_section_by_name(".symtab")
                result = {}
                for name, fmt in spec.items():
                    matches = symbols.get_symbol_by_name(name) or []
                    check(len(matches) == 1, f"missing or ambiguous {name}")
                    offset = matches[0]["st_value"] - 0x400000
                    size = struct.calcsize("<"+fmt)
                    check(0 <= offset <= 0x400000-size and matches[0]["st_size"] == size,
                          f"unexpected private symbol {name}")
                    ram.seek(spaces - base + slot*0x415000 + 0x5000 + offset)
                    result[name] = list(struct.unpack("<"+fmt, ram.read(size)))
                products.append(result)
    a,b,c = products
    deadlines, observed = a["report_deadline_ns"], a["report_observed_ns"]
    ok = (a["private_progress"] == [3] and a["private_result"] == [7] and all(
          0 < d <= t for d,t in zip(deadlines, observed)) and
          all(deadlines[i] >= observed[i-1]+40_000_000 for i in (1,2)) and
          b["private_progress"] == [536870912] and b["private_result"] == [2097152*math.comb(8,i) for i in range(9)]
          and c["private_progress"] == [1073741824] and c["private_result"] == [5864240971776])
    return {"private_values":products, "match":ok}


def guest(root, mode, apps, linked):
    launch = load(root/"launch.json")
    argv = launch["argv"]
    prefix = ["/usr/bin/qemu-system-aarch64", *FLAGS, "-qmp","stdio","-serial"]
    check(launch["mode"] == mode and launch["kernel"] == ELF_HASH and launch["qemu"] == QEMU and
          argv[:len(prefix)] == prefix, "unexpected cold launch")
    rest = argv[len(prefix):]
    check(len(rest) == (9 if mode == "observed" else 7) and rest[0].startswith("file:") and
          rest[1] == "-kernel" and rest[3:6] == ["-d","int","-D"] and
          (mode == "control" or rest[7] == "-plugin"), "unexpected trace/observation arguments")
    out = load(root/"outcome.json")
    check(out["done"] and not out.get("error"), "capture incomplete")
    uart = root/"uart"
    health = read_uart(uart,[7,70,179])
    lines = uart.read_bytes().splitlines()
    for prefix, app in zip(("", "PEER ", "PEER2 "), APP_HASHES):
        check(lines.count(f"NATIVE EL0 {prefix}ELF {app}".encode()) == 1, "guest ELF digest mismatch")
    rows = [list(map(int,l.split()[3:])) for l in lines if l.startswith(b"NATIVE HIERARCHY EVENT ")]
    config, = [list(map(int,l.split()[3:])) for l in lines if l.startswith(b"NATIVE HIERARCHY CONFIG ")]
    check(config[2:4] == [62500000,P], "unexpected guest counter or refill grid")
    gaps = [r[3]-p[3] for p,r in zip(rows,rows[1:])]
    expiry = [(r[0],r[3]-r[13]) for r in rows if r[1:3] == [1,30]]
    regular = [(r[0],max(0,r[18])) for r in rows if r[1:3] == [1,30] and r[8]<3 and r[13]==r[16]]
    return {"root":str(root.resolve()), "launch":launch, "outcome":out, "health":health,
            "uart_sha256":digest(uart),"products":product_data(root, apps, linked),
            "timing":{"detail":health["detail"], "limit_ticks":P//8,"decision_limit_ticks":8*P,
                      "max_entry_gap_ticks":max(gaps,default=None),"decision_gap_violations":[r[0] for r,g in zip(rows[1:],gaps) if g>8*P],
                      "irq_expiry_violations":[p for p in expiry if not 0<=p[1]<=P//8],
                      "limited_grants":len(regular),"positive_overrun_sum":sum(x for _,x in regular),
                      "overrun_violations":[p for p in regular if p[1]>P//8]},
            "reporter_sleep_rows":[r[0] for r in rows if r[1]==2 and r[2]==7 and r[8]==0 and r[32]],
            "reporter_wake_rows":[r[0] for r in rows if r[11]&1],
            "peer_dispatch_while_reporter_asleep":[r[0] for r in rows if r[32] and r[9] in (1,2)]}


def projected(sample, tick, ppb, error):
    delta = (tick-sample["guest_ticks"])*16
    width = (abs(delta)*ppb + 999999999)//1000000000 + error
    a,b = sample["host_ns"]
    return [a+delta-width,b+delta+width]


def intersect(bounds):
    lo, hi = max(a for a,_ in bounds), min(b for _,b in bounds)
    return [lo,hi] if lo <= hi else None


def projection(anchors, i, resolution):
    a = anchors[i]
    d,e = a["due_ticks"], a["samples"][1]["guest_ticks"]
    ordered = [s for anchor in anchors for s in anchor["samples"]]
    before = [s["host_ns"][0]-resolution for s in ordered if s["guest_ticks"]<=d]
    after = [s["host_ns"][1]+resolution for s in ordered if s["guest_ticks"]>=d]
    causal = [max(before),min(after)] if before and after and max(before)<=min(after) else None
    # Guest ticks can tie. Only strictly ordered ticks constrain D across different reads.
    if any(s["guest_ticks"] == d for s in ordered):
        before = [s["host_ns"][0]-resolution for s in ordered if s["guest_ticks"]<d]
        after = [s["host_ns"][1]+resolution for s in ordered if s["guest_ticks"]>=d]
        causal = [max(before), min(after)] if before and after else None
    direct = [a["samples"][1]["host_ns"][0]-resolution, a["samples"][1]["host_ns"][1]+resolution]
    result = {"causal_due_ns":causal,"irq_ns":direct,"due_ns":causal,"basis":"causal only"}
    neighbors = anchors[max(0,i-1):i]+anchors[i+1:i+2]
    if len(neighbors)!=2 or any(n["outcome"] in (2,3,4) for n in neighbors):
        return result
    # Read values identify a 16 ns guest tick bin; retain a whole bin
    # at each projected endpoint as well as host resolution and rate error.
    error = 32
    basis = [s for n in neighbors for s in n["samples"]]
    def model(t):
        return intersect([projected(s,t,0,error+resolution) for s in basis])
    checks = []
    for s in a["samples"]:
        predicted = model(s["guest_ticks"])
        measured = [s["host_ns"][0]-resolution,s["host_ns"][1]+resolution]
        checks.append(bool(predicted and intersect([predicted,measured])))
    due, irq = model(d), model(e)
    result.update(anchor_projections=[{"seq":s["seq"],"due_ns":projected(s,d,0,error+resolution),
                                       "irq_ns":projected(s,e,0,error+resolution)} for s in basis],
                  held_out_overlap=checks, nominal_due_ns=due, nominal_irq_ns=irq)
    return result

def task_series(host, tid):
    result = []
    identity = None
    for _,begin,end,tasks in host["samples"]["rows"]:
        hits = [t for t in tasks if t["tid"]==tid]
        check(len(hits)==1,"sample TID missing/duplicated")
        t = hits[0]
        fields = [t[k] for k in ("start_before","schedstat","switches","start_after")]
        check(all(len(v)==4 and v[3] is None and begin<=v[0]<=v[1]<=end for v in fields)
              and all(a[1]<=b[0] for a,b in zip(fields,fields[1:])),"failed or overlapping task reads")
        st, counter, switches, finish = fields
        if identity is None:
            identity = st[2]
        check(st[2]==finish[2]==identity and len(counter[2])==3 and len(switches[2])==2,
              "task identity or accounting fields changed")
        values = [*counter[2],*switches[2]]
        check(all(type(v) is int and v>=0 for v in values),"invalid task counters")
        if result:
            check(result[-1][1]<=counter[0] and all(a<=b for a,b in zip(result[-1][2],values)),
                  "task counter or sample order regressed")
        result.append((counter[0],counter[1],values))
    return {"tid":tid,"starttime":identity,"rows":result}


def contribution(series, start, stop, witness, calibrated, resolution=0):
    inside = [(a-resolution,b+resolution,v) for a,b,v in series["rows"]
              if start<=a-resolution and b+resolution<=stop]
    out = {"tid":series["tid"],"starttime":series["starttime"],"post_due_interval_ns":[start,stop],
           "inside_samples":len(inside),"runnable_wait_lower_ns":0,"runnable_wait_upper_ns":max(0,stop-start)}
    if len(inside)<2:
        return out
    a,b = inside[0],inside[-1]
    cpu,wait,slices = [y-x for x,y in zip(a[2][:3],b[2][:3])]
    # A callback proves the emitting TID ran at some time inside its bracket.
    # No wait booked after the first read can have begun before that execution.
    carry = max(0,start-witness[0]) if witness and witness[1]<=a[0] else None
    out.update(schedstat_read_bounds_ns=[a[:2],b[:2]],charged_cpu_ns=cpu,booked_wait_ns=wait,
               timeslices=slices,carry_in_upper_ns=carry,
               uncovered_edges_upper_ns=(a[1]-start)+(stop-b[0]))
    if calibrated and carry is not None:
        lower = max(0,wait-carry)
        if lower<=stop-start:
            out["runnable_wait_lower_ns"] = lower
        else:
            out["limitation"] = "counter increment incompatible with the bounded interval"
    return out


def analyze(args):
    idle,addr = sites(args.kernel)
    with args.kernel.open("rb") as stream:
        linked = elf_layout(ELFFile(stream))
    control = guest(args.control,"control",args.apps,linked)
    observed = guest(args.observed,"observed",args.apps,linked)
    host = load(args.observed/"host.json")
    pre = load(args.preflight)
    check(control["launch"]["platform"]==observed["launch"]["platform"]==pre["platform"],
          "acquisition/preflight platforms differ")
    check(host["version"]==1 and host["clock"]=="CLOCK_MONOTONIC ns" and host["cpu0_tid"]>0,
          "unexpected host clock or CPU0 identity")
    marker = correlate(argparse.Namespace(markers=args.markers,uart=args.observed/"uart",tid=host["cpu0_tid"]),idle,addr)
    limits = list(marker["limitations"])
    if observed["health"]["instrumentation"]!="valid":
        limits.append("guest instrumentation incomplete")
    if host["error"]:
        limits.append("host sampler error")
    for key in ("samples","qmp"):
        buf=host[key]
        check(buf["total"]==len(buf["rows"])+buf["dropped"] and buf["dropped"]>=0,"host buffer count mismatch")
        if buf["dropped"]:
            limits.append(f"{key} lost records")
    qmp = host["qmp"]["rows"]
    setup = [r for r in qmp if r[:3]==["setup",None,"query-cpus-fast"]]
    check(len(setup)==1 and all(len(r)==7 and r[4]<=r[5] for r in qmp)
          and all(a[5]<=b[4] for a,b in zip(qmp,qmp[1:])),"invalid QMP ordering")
    running = [r for r in qmp if r[0]=="running"]
    if (len(running)!=4*observed["outcome"]["rounds"] or any(r[6] for r in qmp) or
        any(r[1]!=i//4 or r[2]!="pmemsave" or r[3]["size"]!=(2496 if i%4==3 else 128)
            for i,r in enumerate(running))):
        limits.append("running QMP cadence or command failure")
    pf = [pre[k] for k in ("before","after_cpu","after_sleep")]
    pf_tids = {r["tid"] for r in pf}
    widths = pre["clock_brackets"]
    if (len(pf_tids)!=1 or not widths or
        not all(len(v)==4 and v[3] is None and v[0]<=v[1] for v in widths) or
        not pre["monotonic"]["monotonic"] or pre["monotonic"]["resolution"]<=0 or
        any(r[k][3] is not None for r in pf for k in ("schedstat","start_before","start_after","switches")) or
        len({r[k][2] for r in pf for k in ("start_before","start_after")})!=1 or
        pf[1]["schedstat"][2][0]<=pf[0]["schedstat"][2][0]):
        limits.append("host clock or CPU accounting preflight incomplete")
    resolution = max(marker["monotonic_resolution_ns"],math.ceil(pre["monotonic"]["resolution"]*1e9))
    series = {}
    try:
        for tid in {host["cpu0_tid"],host["qemu_pid_main_tid"]}:
            series[tid] = task_series(host,tid)
    except ValueError as exc:
        limits.append(str(exc))
    # The stock preflight measures CPU advancement and a voluntary sleep. It
    # cannot independently qualify runnable-wait accounting under contention.
    wait = None
    if args.wait_calibration:
        try:
            from native_idle_wait_calibration import validate_wait_calibration
            receipt = load(args.wait_calibration)
            for key in ("platform","uid","boot_id"):
                check(receipt[key] == pre[key] == host[key], f"wait calibration {key} differs")
            check(receipt["allowed_before"] == receipt["allowed_after"] ==
                  pre["allowed_cpus"] == host["allowed_cpus"], "wait calibration allowed CPUs differ")
            derived = validate_wait_calibration(receipt,pre["platform"])
            wait = {"path":str(args.wait_calibration.resolve()),"sha256":digest(args.wait_calibration),
                    "trial_bounds":derived}
        except (ImportError,OSError,ValueError,TypeError,KeyError,IndexError) as exc:
            wait = {"insufficient":str(exc)}
    records = []
    anchors = marker["anchors"]
    ticks = [s["guest_ticks"] for a in anchors for s in a["samples"]]
    if any(b<a for a,b in zip(ticks,ticks[1:])):
        limits.append("guest ticks regress between marker pairs")
    for i,a in enumerate(anchors):
        if a["outcome"]!=2:
            continue
        model = projection(anchors,i,resolution)
        model["reference_tick_ns"] = 16
        r = dict(a,clock=model,lag_ticks=a["samples"][1]["guest_ticks"]-a["due_ticks"],
                 runnable_wait_lower_ns=0, verdict="insufficient clock or accounting coverage")
        r["over_limit_ticks"] = r["lag_ticks"] - P//8
        r["masked_boundary_vs_due_ticks"] = a["samples"][0]["guest_ticks"] - a["due_ticks"]
        if not limits and model["due_ns"] and model["irq_ns"]:
            start,stop=model["due_ns"][1],model["irq_ns"][0]
            if start<=stop:
                witness = max((s["host_ns"] for anchor in anchors for s in anchor["samples"]
                               if s["host_ns"][1]+resolution<=start),key=lambda b:b[0],default=None)
                if witness:
                    witness=[witness[0]-resolution,witness[1]+resolution]
                r["threads"] = [contribution(s,start,stop,witness if tid==host["cpu0_tid"] else None,wait is not None and "trial_bounds" in wait,resolution)
                                for tid,s in sorted(series.items())]
                lower=max(t["runnable_wait_lower_ns"] for t in r["threads"])
                r.update(runnable_wait_lower_ns=lower,verdict="positive post-deadline runnable contribution conditional on supplied calibration"
                         if lower else "no positive post-deadline runnable lower bound")
                r["qmp_overlaps"]=[{"index":j,"phase":v[0],"round":v[1],"command":v[2],"host_ns":v[4:6]}
                                   for j,v in enumerate(qmp) if v[4]<=stop and start<=v[5]]
        records.append(r)
    h=observed["health"]["health"]
    verdict=("no observed natural idle refill miss" if h["late"]==0 and h["scored"] and
             observed["health"]["instrumentation"]=="valid" else "unresolved observation")
    if h["late"] != len(records):
        limits.append("complete row-to-marker coverage of natural late entries unavailable")
    if records:
        verdict="see event bounds"
    return {"control":control,"observed":observed,"preflight_path":str(args.preflight.resolve()),
            "preflight_sha256":digest(args.preflight),"preflight_empty_widths_ns":[v[1]-v[0] for v in widths],
            "marker_path":str(args.markers.resolve()),"marker_join":marker,"clock_bound":"causal only; independent across-idle rate bound unavailable","wait_calibration":wait,
            "host":{"path":str((args.observed/"host.json").resolve()),"sha256":digest(args.observed/"host.json"),
                    "main_tid":host["qemu_pid_main_tid"],"cpu_tid":host["cpu0_tid"],"qmp_setup":setup,
                    "sample_count":host["samples"]["total"],"sample_drops":host["samples"]["dropped"],
                    "skipped_periods":host["skipped_periods"],"sampler_cpu_ns":host["sampler_cpu_ns"]},
            "limitations":limits,"observed_verdict":verdict,"natural_late_records":records,
            "historical":{"late_ticks":[4201556,6487068,9989268],"limit_ticks":P//8,"attribution":"unresolved"},
            "provider":"No qualified emitter-side assertion timestamp; QMP overlap identifies activity only."}


def main():
    p=argparse.ArgumentParser(description=__doc__,formatter_class=argparse.RawDescriptionHelpFormatter)
    for name in ("control","observed","preflight","kernel","markers"):
        p.add_argument("--"+name,type=Path,required=True)
    p.add_argument("--apps",nargs=3,type=Path,required=True,metavar=("REPORTER","HISTOGRAM","ENERGY"))
    p.add_argument("--wait-calibration",type=Path,help="raw same-boot wait-calibration.json produced by native_idle_wait_calibration.py")
    try:
        result=analyze(p.parse_args())
    except (OSError,ValueError,TypeError,KeyError,IndexError,struct.error) as exc:
        print(json.dumps({"observed_verdict":"insufficient acquisition evidence","reason":str(exc)},indent=2))
        return 2
    print(json.dumps(result,indent=2))
    return 0


if __name__=="__main__":
    sys.exit(main())
