"""Conditional FS-M3-01 single-core analysis. All intervals in microseconds."""
import argparse
import json
from pathlib import Path

from fp_response import _fields, _integer, _response, _unique_fields


def numbers(row, names, label):
    _fields(row, set(names.split()), set(), label)
    for key, value in row.items():
        _integer(value, 0, f"{label}.{key}")


ASSUMPTIONS = {
    "scope": "FS-M3-01 virtual bench",
    "cores": 1, "initially_idle": True, "unit": "us",
    "absolute_phase_offsets": [0, 2500], "control_period_deadline": 5000,
    "sender_period_deadline": 10000, "higher_priority": "control",
    "same_worker_absolute_continuation": True, "no_early_activation": True,
    "no_additional_suspensions_or_skips": True, "all_irq_sources_bounded": True,
    "irq_finish_includes_pending_and_masking": True,
    "dispatch_and_instrumentation_accounted_once": True,
    "uncertainty_covers_both_endpoints": True,
}


def analyze(config, policy):
    if policy not in ("rm", "dm"):
        raise ValueError("unsupported fixed-priority policy")
    _fields(config, {"a", "b", "sender", "timer", "other_irqs",
                     "uncertainty", "irq_entry_lateness"}, set(), "config")
    uncertainty = _integer(config["uncertainty"], 0, "uncertainty")
    entry = _integer(config["irq_entry_lateness"], 0, "irq_entry_lateness")
    sources = []
    if not isinstance(config["other_irqs"], list):
        raise ValueError("other_irqs must enumerate all additional sources")
    if len(config["other_irqs"]) > 64:
        raise ValueError("analysis supports at most 64 additional IRQ sources")
    for name in ("a", "b", "sender"):
        row = config[name]
        numbers(row, "service dispatch jitter blocking", name)
        if not row["service"]:
            raise ValueError(f"{name} requires positive service")
    for irq in [config["timer"], *config["other_irqs"]]:
        numbers(irq, "period cost finish", "irq")
        if not 0 < irq["cost"] <= irq["finish"] or not irq["period"]:
            raise ValueError("IRQ requires positive period and 0 < cost <= finish")
        sources.append(dict(period=irq["period"], cost=irq["cost"], jitter=irq["finish"]))
    if config["timer"]["period"] != 2500:
        raise ValueError("fixture requires one timer activation per phase")
    result = dict(policy=policy, unit="us", irq_entry=entry + uncertainty)
    result["assumptions"] = dict(ASSUMPTIONS)
    reasons = result["unproved_limits"] = []
    if result["irq_entry"] > 500:
        reasons.append("irq_entry_500us")
    phases = []
    for name in ("a", "b", "sender"):
        row = config[name]
        cost = row["service"] + row["dispatch"]
        limit = 10000 if name == "sender" else 2500
        task = dict(id=name, cost=cost, jitter=row["jitter"],
                    blocking=row["blocking"], deadline=limit)
        interferers = sources + phases if name == "sender" else sources
        response = _response(task, interferers, limit + 1)
        result[name] = response
        raw = response["bound"]
        if raw is None:
            result["status"] = "refused" if response["status"] == "deadline_exceeded" else "unknown"
            reasons.append(f"{name}_response_window")
            return result
        upper = raw + uncertainty
        response["upper_with_uncertainty"] = upper
        if name == "sender":
            if upper > 10000:
                reasons.append("sender_deadline_10000us")
        else:
            if upper > 1000:
                reasons.append(f"{name}_decision_1000us")
            if raw >= 2500:
                reasons.append(f"{name}_phase_overlap")
            phases.append(dict(period=5000, jitter=raw, cost=cost))
    result["check_gap"] = 2500 + max(p["jitter"] for p in phases) + uncertainty
    result["job_finish"] = 2500 + phases[1]["jitter"] + uncertainty
    result["worker_service_plus_dispatch"] = dict(
        control_job=sum(p["cost"] for p in phases),
        per_nominal_10ms=2 * sum(p["cost"] for p in phases) + cost)
    if result["check_gap"] > 5000:
        reasons.append("check_gap_5000us")
    if result["job_finish"] > 5000:
        reasons.append("job_deadline_5000us")
    result["status"] = "refused" if reasons else "conditional"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", type=Path)
    parser.add_argument("policy", choices=("rm", "dm"))
    args = parser.parse_args()
    try:
        config = json.loads(args.config.read_text(encoding="utf-8"), object_pairs_hook=_unique_fields)
        result = analyze(config, args.policy)
        output = json.dumps(result, indent=2)
    except RecursionError:
        result = dict(status="unknown", reason="configuration nesting exceeds parser limit")
        output = json.dumps(result, indent=2)
    except (ValueError, OSError) as exc:
        result = dict(status="unknown", reason=str(exc))
        output = json.dumps(result, indent=2)
    print(output)
    return {"conditional": 0, "refused": 1, "unknown": 2}[result["status"]]


if __name__ == "__main__":
    raise SystemExit(main())
