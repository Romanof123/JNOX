"""Check effective kernel selectors through Cargo's package and build-script entry points."""
import os
from pathlib import Path
import subprocess
import tempfile

KERNEL = Path(__file__).resolve().parents[1] / "kernel"


def main():
    env = {k: v for k, v in os.environ.items() if not k.startswith("JNOX_")}
    with tempfile.TemporaryDirectory() as target:
        env["CARGO_TARGET_DIR"] = target

        def cfg(features="", *, board="virt", reject=None, **selectors):
            cmd = ["cargo", "rustc", "--release"]
            if features == "all":
                cmd.append("--all-features")
            elif features:
                cmd.extend(["--features", features])
            cmd.extend(["--", "--print", "cfg"])
            run = subprocess.run(cmd, cwd=KERNEL, env=dict(env, JNOX_BOARD=board, **selectors),
                                 text=True, capture_output=True)
            if reject is not None:
                if run.returncode == 0 or reject not in run.stderr:
                    raise AssertionError(f"{cmd} did not reject with {reject}: {run.stderr}")
                return None
            if run.returncode:
                raise RuntimeError(run.stderr)
            return {line for line in run.stdout.splitlines() if line.startswith("jnox_")}

        def selected(features, backend, ordinary=False):
            effective = cfg(features)
            if f'jnox_bench="{backend}"' not in effective or ("jnox_ordinary_partition" in effective) != ordinary:
                raise AssertionError(f"unexpected selector for {features!r}: {effective}")
            return effective

        cursor_error = "JNOX_PARTITION_WINDOW=cursor-v1 requires a native P/Q partition"
        cfg(JNOX_PARTITION_WINDOW="cursor-v1", reject=cursor_error)
        cfg(JNOX_PARTITION_WINDOW="cursor-v1", JNOX_PROFILE="native",
            JNOX_APP="weighted-share", reject=cursor_error)
        cfg(JNOX_PARTITION_WINDOW="cursor-v2", JNOX_PROFILE="native",
            JNOX_APP="partition-p", reject=cursor_error)
        selected("", "boot")
        selected("partition-bench", "partition")
        selected("ordinary-partition-p", "partition", True)
        selected("ordinary-partition-q", "partition", True)
        cfg("ordinary-partition-p,ordinary-partition-q", reject="select exactly one ordinary partition table")
        cfg("ordinary-partition-q", board="pi4", reject="ordinary partition requires virt")
        selected("partition-bench,ordinary-partition-q", "partition")
        selected("cbs-bench,ordinary-partition-p", "cbs")
        coop = selected("coop-bench", "coop")
        if selected("all", "coop") != coop:
            raise AssertionError("all-features changed the legacy cooperative selector")


if __name__ == "__main__":
    main()
