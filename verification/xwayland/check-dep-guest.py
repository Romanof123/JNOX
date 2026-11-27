"""Check original function entry PCs, terminal status and native font bytes."""
import json
import re
import sys
from pathlib import Path

uart = Path(sys.argv[1])
mode = sys.argv[2]
expected = {"font": {"BuiltinFileOpen", "inflate"},
            "xkb": {"XkbRF_Create", "XkbRF_Free", "XkbModIndexText"}}[mode]
symbols = json.loads(uart.with_suffix(".symbols.json").read_text())
if set(symbols) != expected:
    sys.exit("wrong symbol filter")
lines = uart.with_suffix(".trace").read_text().splitlines()
entries = set()
for i, line in enumerate(lines):
    match = re.match(r"^Trace 0: 0x[0-9a-f]+ \[[0-9a-f]+/([0-9a-f]+)/", line, re.I)
    if match and not (i+1 < len(lines) and lines[i+1].startswith("Stopped execution of TB chain before")):
        entries.add(int(match[1], 16))
for name, (pc, _) in symbols.items():
    if pc not in entries:
        sys.exit("missing executed entry " + name)
raw = uart.read_bytes().replace(b"\r", b"")
if re.findall(rb"^NATIVE EL0 EXIT (.*)$", raw, re.M) != [b"0"]:
    sys.exit("wrong terminal status")
if mode == "font" and re.findall(rb"^FONT ROWS (.*)$", raw, re.M) != [b"0000040a1111111f1111110000"]:
    sys.exit("wrong glyph rows")
print(mode, "verified", *sorted(symbols))
