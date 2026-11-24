"""Check linked task work and ordering constraints independently of scheduler internals."""

MAX_BYTES = 512


def validate(data, prefix):
    lines = data.split(b"\n")
    if lines[:2] != [b"JNOX M1 BOOT", b"JNOX M1 READY"] or lines[-1:] != [b""]:
        return False
    markers = lines[2:-1]
    tag = prefix + b" "
    if not all(line.startswith(tag) for line in markers):
        return False
    events = [line[len(tag):] for line in markers]
    if prefix == b"FIXED FIFO":
        chain = [b"START", b"A START", b"B YIELD", b"A RESUME", b"A DONE", b"B SLEEP", b"B DONE"]
    elif prefix == b"FIXED RR":
        chain = [b"START", b"A START", b"A DONE", b"B DONE"]
    else:
        return False
    required = [*chain, b"HIGH"]
    if prefix == b"FIXED RR":
        required.append(b"B START")
    if sorted(events) != sorted(required) or events[0] != b"START":
        return False
    if [e for e in events if e in chain] != chain:
        return False
    if prefix == b"FIXED FIFO":
        # HIGH observes A's busy work. A waits for that observation before returning.
        return events.index(b"A RESUME") < events.index(b"HIGH") < events.index(b"A DONE")
    # B must publish progress before A can return. HIGH's initial deadline
    # precedes the first RR expiry, but may already be due when A starts.
    return events.index(b"HIGH") < events.index(b"B START") < events.index(b"A DONE")
