"""Materialize the actual 1.23.1 outgoing descriptor call and its OS implementation."""
import hashlib
import json
import sys
import tarfile
from pathlib import Path

archive, out = map(Path, sys.argv[1:])
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
if digest != "158ec49af498f2558c7fbf7e8b070d010d4e270cc6076003a18a6c813f87e244":
    raise ValueError("Wayland 1.23.1 source digest")
with tarfile.open(archive) as tar:
    def source(name):
        return tar.extractfile("wayland-1.23.1/src/" + name).read().decode()
    os_c = source("wayland-os.c")
    connection = source("connection.c")

def through(text, first, last):
    if text.count(first) != 1:
        raise ValueError(first)
    start = text.index(first)
    return text[start:text.index(last, start) + len(last)]

pointer = through(os_c, "int (*wl_fcntl)(", ";")
flags = through(os_c, "static int\nset_cloexec_or_close(", "\n}")
duplicate = through(os_c, "int\nwl_os_dupfd_cloexec(", "\n}")
call = through(connection, "dup_fd = wl_os_dupfd_cloexec(", ";")
output = ("/* Generated from pinned Wayland 1.23.1. */\n" + os_c[:os_c.index("#define _GNU_SOURCE")] +
          "#include <errno.h>\n#include <fcntl.h>\n#include <unistd.h>\n" +
          pointer + "\n" + flags + "\n" + duplicate +
          "\nstatic int upstream_outgoing_fd(int fd) { int dup_fd; " + call + " return dup_fd; }\n")
(out/"upstream-dup.inc").write_text(output)
(out/"upstream-source.json").write_text(json.dumps(
    {"archive_sha256":digest, "connection_expression":call,
     "include_sha256":hashlib.sha256(output.encode()).hexdigest()},indent=2))
