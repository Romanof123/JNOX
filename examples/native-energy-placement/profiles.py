"""Write the frozen #357 model inputs from the versioned base profile."""
import argparse
import copy
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new directory")
    args = parser.parse_args()
    source = Path(__file__).with_name("base.json").read_bytes()
    base = json.loads(source)
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / "base.json").write_bytes(source)
    for name in ("thermal", "shared", "fallback", "empty", "deadline", "zero-mask", "missing-limit"):
        profile = copy.deepcopy(base)
        if name == "thermal":
            profile["limits"][0]["thermal_capacity"] = 0
        elif name == "shared":
            profile["limits"][0]["shared_load"] = 3
        elif name in ("fallback", "empty"):
            if name == "fallback":
                profile["fallback"] = profile["points"][1]
            profile["points"] = []
        elif name == "deadline":
            for limit in profile["limits"]:
                limit["thermal_capacity"] = 1
        elif name == "zero-mask":
            profile["allowed_cpus"] = 0
        elif name == "missing-limit":
            profile["limits"].pop()
        (args.output / (name + ".json")).write_bytes((json.dumps(profile, indent=2) + "\n").encode("ascii"))


if __name__ == "__main__":
    main()
