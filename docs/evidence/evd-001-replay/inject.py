from pathlib import Path
p = Path(__file__).resolve().parent / "mutant/kernel/src/start.S"
content = p.read_bytes()
old = b"str xzr, [x0], #8"
assert content.count(old) == 1
p.write_bytes(content.replace(old, b"add x0, x0, #8"))
