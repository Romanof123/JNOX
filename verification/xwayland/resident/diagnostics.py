"""Allowlisted diagnostic fields from the resident guest's normal console."""
import re
import struct


ROLES = ("compositor", "xwayland", "tiles", "broker", "mosaic")
BANNERS = ("ELF", "PEER ELF", "PEER2 ELF", "PEER3 ELF", "PEER4 ELF")
LIFECYCLE = re.compile(rb"(?m)^NATIVE EL0 (RETIRED|EXIT) PID ([0-4]) (-?[0-9]{1,10})\r?\n")
PHASES = re.compile(rb"(?m)^NATIVE EL0 (VALIDATE|STACK|ENTERED) PID ([0-4])\r?\n")
FAULT = re.compile(rb"(?m)^NATIVE EL0 FAULT ([0-9]{1,3})\r?\n")
TRAP = re.compile(rb"(?m)^NATIVE EL0 TRAP PID ([0-4]) ([0-9]{1,2})\r?\n")
TRAP_PC = re.compile(rb"(?m)^NATIVE EL0 TRAP_PC PID ([0-4]) ([0-9]{1,8})\r?\n")
SERVER_PHASE = re.compile(rb"(?m)^JNOX XWL (BROKER|ARGUMENTS|OS|LISTENERS|OUTPUT|DISPATCH)\r?\n")
MOSAIC_ERRORS = {
    100: "arguments", 101: "socket", 102: "connect", 103: "write-setup", 104: "setup-reply",
    105: "open-window", 106: "read-event", 107: "protocol-event", 108: "draw",
    109: "request-destroy", 110: "unexpected-destroy", 114: "authorization-denied",
    115: "node-identity", 116: "authority-record",
}
# Match only static strings from the pinned DIX, libxtrans and native sources.
# Pathnames, message suffixes and general-purpose register values are untrusted.
SERVER_MESSAGES = {
    b"JNOX Xwayland could not select its socket broker": "broker-selection",
    b"Unrecognized option:": "arguments",
    b"Could not create lock file in": "display-lock",
    b"Could not create server lock file:": "display-lock",
    b"failed to allocate poll structure": "poll-allocation",
    b"Cannot install server SIGPIPE or SIGHUP disposition": "signal-setup",
    b"Cannot install server SIGINT or SIGTERM disposition": "signal-setup",
    b"Failed to establish all listening sockets": "listener-incomplete",
    b"Cannot establish any listening sockets": "listener-unavailable",
    b"Failed to find a socket to listen on": "listener-unavailable",
    b"Failed to create listening socket array": "listener-allocation",
    b"SocketOpenCOTSServer: Unable to open socket": "xtrans-socket-open",
    b"SocketOpenCOTSServer: Unable to determine socket type": "xtrans-socket-type",
    b"SocketOpen: socket() failed": "xtrans-socket-open",
    b"MakeAllCOTSServerListeners: failed to open listener": "xtrans-open-listener",
    b"MakeAllCOTSServerListeners: failed to create listener": "xtrans-create-listener",
    b"SocketUNIXCreateListener: mkdir(": "xtrans-socket-directory",
    b"SocketUNIXCreateListener: path too long": "xtrans-socket-path",
    b"SocketUNIXCreateListener: ...SocketCreateListener() failed": "xtrans-unix-listener",
    b"SocketCreateListener: failed to bind listener": "xtrans-bind",
    b"SocketCreateListener: listen() failed": "xtrans-listen",
    b"mkdir: ERROR: (l)stat failed": "xtrans-directory-stat",
    b"mkdir: ERROR: Cannot create": "xtrans-directory-create",
    b"mkdir: ERROR: fstat failed": "xtrans-directory-fstat",
    b"mkdir: ERROR: Owner of": "xtrans-directory-owner",
    b"mkdir: ERROR: Mode of": "xtrans-directory-mode",
    b"Cannot initialize native X authorization": "authority-init",
    b"could not connect to wayland server": "wayland-connect",
    b"Couldn't add screen": "wayland-screen",
    b"no screens found": "wayland-screen",
    b"failed to create root window": "root-window",
    b"could not open default font": "default-font",
    b"could not open default cursor font": "cursor-font",
    b"Fatal server error:": "fatal-server-error",
    b"JNOX GRAPHICS ABORT": "graphics-abort",
}


def console(events, identity, receipt):
    events = events[:events.rfind(b"\n") + 1]
    diagnostic = receipt.setdefault("diagnostic", {})
    diagnostic["images_announced"] = [
        role for role, banner in zip(ROLES, BANNERS)
        if re.search(rb"(?m)^NATIVE EL0 " + banner.encode() + b" " +
                     re.escape(identity["elf_sha256"][role].encode()) + rb"\r?\n", events)
    ]
    for marker, name in ((b"VALIDATE", "validating"), (b"STACK", "constructing_stack"),
                         (b"ENTERED", "first_syscall")):
        pids = [int(pid) for phase, pid in PHASES.findall(events) if phase == marker]
        if pids:
            diagnostic[name] = [ROLES[pid] for pid in pids]
    names = sorted({name for prefix, name in SERVER_MESSAGES.items() if prefix in events})
    if names:
        diagnostic["server_messages"] = names
    phases = SERVER_PHASE.findall(events)
    if phases:
        diagnostic["last_server_phase"] = phases[-1].decode("ascii").lower()
    faults = FAULT.findall(events)
    if faults:
        diagnostic["kernel_fault"] = int(faults[-1])
    traps = TRAP.findall(events)
    if traps:
        pid, ec = map(int, traps[-1])
        if ec < 64:
            diagnostic["el0_exception"] = {"role": ROLES[pid], "class": ec}
            for row_pid, address in TRAP_PC.findall(events):
                pc = int(address)
                if int(row_pid) == pid and 0x400000 <= pc < 0x800000 and pc % 4 == 0:
                    diagnostic["el0_exception"]["pc"] = hex(pc)
    exits = {}
    terminal = {}
    retired = {}
    for kind, raw_pid, raw_code in LIFECYCLE.findall(events):
        pid, code = int(raw_pid), int(raw_code)
        source = terminal if kind == b"EXIT" else retired
        if pid in source or not -(1 << 31) <= code < 1 << 31 or exits.get(pid, code) != code:
            raise ValueError("inconsistent guest exit record")
        source[pid] = exits[pid] = code
    receipt["exits"] = exits
    receipt["terminal_exits"] = terminal
    if exits.get(4) in MOSAIC_ERRORS:
        diagnostic["mosaic_status"] = MOSAIC_ERRORS[exits[4]]
    return exits


def executable_ranges(path):
    """Read only the executable PT_LOAD addresses in a packaged ELF64 image."""
    with path.open("rb") as image:
        header = image.read(64)
        if len(header) != 64 or header[:6] != b"\x7fELF\x02\x01":
            raise ValueError("invalid package ELF")
        offset, = struct.unpack_from("<Q", header, 32)
        size, count = struct.unpack_from("<HH", header, 54)
        if size != 56 or not 0 < count < 256:
            raise ValueError("invalid ELF program header table")
        image.seek(offset)
        ranges = []
        for _ in range(count):
            kind, flags, _, va, _, _, length, _ = struct.unpack("<II6Q", image.read(size))
            if kind == 1 and flags & 1:
                ranges.append((va, va + length))
        return ranges


if __name__ == "__main__":
    import argparse
    import json
    from pathlib import Path

    parser = argparse.ArgumentParser(description="Classify the current private UART, emitting allowlisted JSON.")
    parser.add_argument("workload", type=Path, help="packaged workload.json")
    parser.add_argument("uart", type=Path, help="existing private UART within the active run")
    args = parser.parse_args()
    with args.uart.open("rb") as source:
        data = source.read(1024 * 1024 + 1)
    if len(data) > 1024 * 1024:
        parser.error("guest console exceeded observation limit")
    result = {}
    console(data, json.loads(args.workload.read_text()), result)
    print(json.dumps(result, indent=2))
