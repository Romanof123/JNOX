"""Host-side fixed-priority response-time analysis in integer ticks.

Assumes an initially idle, fully preemptive single core and independent
sporadic tasks without self-suspension. The caller supplies blocking bounds.
All modeled IRQs can preempt every task. Dispatch is charged per task job.
Results describe this workload model, not runtime or hardware guarantees.
"""

import argparse
import json
from pathlib import Path
import sys


def _fields(value, required, optional, label):
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be an object")
    missing = required - value.keys()
    unknown = value.keys() - required - optional
    if missing or unknown:
        raise ValueError(f"{label} has missing {missing} or unknown {unknown} fields")


def _integer(value, minimum, label):
    if type(value) is not int or value < minimum:
        raise ValueError(f"{label} must be an integer >= {minimum}")
    return value


def _jobs(rows, kind, seen):
    if not isinstance(rows, list) or (kind == "tasks" and not rows):
        raise ValueError(f"{kind} must be a list" + (" with tasks" if kind == "tasks" else ""))
    required = {"id", "period", "wcet"}
    optional = {"jitter"}
    if kind == "tasks":
        required.add("deadline")
        optional.update({"blocking", "dispatch"})
    jobs = []
    for index, row in enumerate(rows):
        label = f"{kind}[{index}]"
        _fields(row, required, optional, label)
        name = row["id"]
        if not isinstance(name, str) or not name.strip() or name in seen:
            raise ValueError(f"{label}.id must be a unique nonempty string")
        seen.add(name)
        job = {"id": name}
        for key in sorted(required - {"id"}):
            job[key] = _integer(row[key], 1, f"{label}.{key}")
        for key in sorted(optional):
            job[key] = _integer(row.get(key, 0), 0, f"{label}.{key}")
        if kind == "tasks" and not job["jitter"] < job["deadline"] <= job["period"]:
            raise ValueError(f"{label} requires jitter < deadline <= period")
        job["cost"] = job["wcet"] + job.get("dispatch", 0)
        jobs.append(job)
    return jobs


def _response(task, interferers, limit):
    base = task["cost"] + task["blocking"]
    window = base
    result = {"id": task["id"], "bound": None}
    # Check each iterate, including the initial value and final allowed update.
    for iteration in range(limit + 1):
        nominal = window + task["jitter"]
        if nominal > task["deadline"]:
            return dict(result, status="deadline_exceeded", witness_iterate=nominal)
        if iteration == limit:
            break
        demand = base
        for job in interferers:
            arrivals = (window + job["jitter"] + job["period"] - 1) // job["period"]
            demand += arrivals * job["cost"]
        if demand == window:
            return dict(result, status="within_bounds", bound=nominal)
        window = demand
    return dict(result, status="inconclusive")


def analyze(config):
    """Return per-task nominal-release-to-finish bounds in priority order."""
    _fields(config, {"policy", "tasks"}, {"irqs", "iteration_limit"}, "config")
    if config["policy"] not in ("rm", "dm"):
        raise ValueError("policy must be rm or dm")
    limit = _integer(config.get("iteration_limit", 1024), 1, "iteration_limit")
    seen = set()
    tasks = _jobs(config["tasks"], "tasks", seen)
    interferers = _jobs(config.get("irqs", []), "irqs", seen)
    rank = "period" if config["policy"] == "rm" else "deadline"
    if len({task[rank] for task in tasks}) != len(tasks):
        raise ValueError(f"{config['policy']} requires distinct {rank} values")
    tasks.sort(key=lambda task: task[rank])
    results = []
    for task in tasks:
        results.append(_response(task, interferers, limit))
        interferers.append(task)
    statuses = {result["status"] for result in results}
    for status in ("deadline_exceeded", "inconclusive", "within_bounds"):
        if status in statuses:
            return {"status": status, "tasks": results}


def _unique_fields(pairs):
    fields = {}
    for key, value in pairs:
        if key in fields:
            raise ValueError(f"duplicate JSON field {key!r}")
        fields[key] = value
    return fields


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", type=Path)
    args = parser.parse_args()
    try:
        config = json.loads(args.config.read_text(encoding="utf-8"), object_pairs_hook=_unique_fields)
        result = analyze(config)
    except (OSError, ValueError) as error:
        print(f"Invalid configuration: {error}", file=sys.stderr)
        return 2
    print(json.dumps(result, indent=2))
    return {"within_bounds": 0, "deadline_exceeded": 1, "inconclusive": 2}[result["status"]]


if __name__ == "__main__":
    sys.exit(main())
