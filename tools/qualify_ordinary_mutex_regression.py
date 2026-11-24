"""Run against the original feature and integration archives from #236."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile
import zipfile

CLI = Path(__file__).with_name("qualify_ordinary_mutex.py")
POSITIONS = ("PI/plain", "PI/traced", "ceiling/plain", "ceiling/traced")
FIXTURES = (
    "001d81b80f4e5635676bd6c2f13b31f03d4077e9ff132f226deec726f79353e7",
    "fa89510c77757edbcbd70c300c69663f5e7220d31a1303f84015c6207db362fd",
)


def check(path, expected_code, expected_lines):
    observed = subprocess.run([sys.executable, str(CLI), str(path)], capture_output=True,
                              text=True, env=None)
    if observed.returncode != expected_code or observed.stdout.splitlines() != expected_lines:
        raise AssertionError((observed.returncode, observed.stdout, observed.stderr))
    if (expected_code == 2) != observed.stderr.startswith("invalid evidence:"):
        raise AssertionError(observed.stderr)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("feature", type=Path)
    parser.add_argument("integration", type=Path)
    args = parser.parse_args()
    for idx, path in enumerate((args.feature, args.integration)):
        if hashlib.sha256(path.read_bytes()).hexdigest() != FIXTURES[idx]:
            raise ValueError(f"wrong frozen fixture: {path}")
        outputs = []
        for label in POSITIONS:
            missed = idx == 1 and label.endswith("/traced")
            outputs.append(label + (" UNQUALIFIED reason=16" if missed else " QUALIFIED"))
        check(path, idx, outputs + [f"qualified={4-2*idx}/4"])

    with zipfile.ZipFile(args.integration) as archive:
        inputs = {f"{app}/{file}": archive.read(f"{app}/{file}")
                  for app in ("ordinary-pi", "ordinary-ceiling")
                  for file in ("plain.uart", "trace.uart", "exceptions.log")}
    plain = "ordinary-pi/plain.uart"
    traced = "ordinary-pi/trace.uart"
    cases = {
        "missing trace": {"ordinary-ceiling/exceptions.log": None},
        "duplicate launch": {plain: inputs[traced]},
        "swapped protocol": {plain: inputs["ordinary-ceiling/plain.uart"],
                             "ordinary-ceiling/plain.uart": inputs[plain]},
        "mismatched trace": {"ordinary-pi/exceptions.log": inputs["ordinary-ceiling/exceptions.log"],
                             "ordinary-ceiling/exceptions.log": inputs["ordinary-pi/exceptions.log"]},
        "truncated miss": {traced: inputs[traced].split(b"K ")[0] + b"MUTEX END\n"},
    }
    with tempfile.TemporaryDirectory() as temp:
        for label, changes in cases.items():
            path = Path(temp) / (label + ".zip")
            records = {**inputs, **changes}
            with zipfile.ZipFile(path, "w") as fixture:
                manifest = []
                for name, data in records.items():
                    if data is not None:
                        fixture.writestr(name, data)
                        manifest.append(f"{hashlib.sha256(data).hexdigest()}  {name}")
                fixture.writestr("SHA256SUMS", "\n".join(manifest))
            check(path, 2, [])
    print("Frozen cohorts and invalid-input controls passed")


if __name__ == "__main__":
    main()
