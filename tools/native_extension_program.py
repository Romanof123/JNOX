"""Parse the native extension's bounded, data-only instruction language."""

WIDTHS = {"ready": (8, 8, 8), "jump": (8,), "propose": (32, 64, 8, 64), "yield": ()}


def parse_program(source):
    operations = []
    for line_number, line in enumerate(source.decode("utf-8").split("\n"), 1):
        # Rust follows Unicode White_Space; Python also recognizes four record separators.
        if any(char in line for char in "\x1c\x1d\x1e\x1f"):
            raise ValueError(f"line {line_number}: invalid token separator")
        fields = line.split()
        if not fields:
            continue
        name, *args = fields
        if name not in WIDTHS or len(args) != len(WIDTHS[name]):
            raise ValueError(f"line {line_number}: unknown operation or wrong operand count")
        values = []
        for arg, width in zip(args, WIDTHS[name]):
            if not (arg.isascii() and arg.isdecimal()):
                raise ValueError(f"line {line_number}: expected an unsigned ASCII decimal operand")
            digits = arg.lstrip("0") or "0"
            if len(digits) > 20 or int(digits) >= (1 << width):
                raise ValueError(f"line {line_number}: operand exceeds u{width}")
            values.append(int(digits))
        operations.append((name, *values))
        if len(operations) > 16:
            raise ValueError("extension exceeds 16 instructions")
    if not operations:
        raise ValueError("empty extension program")
    for pc, (name, *args) in enumerate(operations):
        if name == "ready" and args[0] > 2:
            raise ValueError(f"instruction {pc}: ready ID must be 0..2")
        targets = args if name == "jump" else args[1:] if name == "ready" else ()
        if any(target >= len(operations) for target in targets):
            raise ValueError(f"instruction {pc}: branch target outside program")
    return [" ".join(map(str, op)) for op in operations]
