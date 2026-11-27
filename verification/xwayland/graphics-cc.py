#!/usr/bin/env python3
"""Use the archived JNOX graphics compiler and linker flags for Autotools."""
import ast
import configparser
import hashlib
import json
import os
from pathlib import Path
import sys

runtime = Path(os.environ["JNOX_GRAPHICS_RUNTIME"]).resolve()
cross = runtime / "jnox.ini"
expected = json.loads((runtime / "manifest.json").read_text())["artifacts"]["jnox.ini"]
if hashlib.sha256(cross.read_bytes()).hexdigest() != expected:
    sys.exit("graphics cross file differs from the runtime manifest")
config = configparser.ConfigParser(interpolation=None)
config.read(cross)
cc = ast.literal_eval(config["binaries"]["c"])
flags = ast.literal_eval(config["built-in options"]["c_args"])
if config["host_machine"]["system"] != "'jnox'" or "--target=aarch64-none-elf" not in cc:
    sys.exit("expected a JNOX AArch64 graphics toolchain")
args = sys.argv[1:]
inspection = len(args) == 1 and args[0] in ("--version", "-v", "-dumpmachine", "-dumpversion", "-print-search-dirs")
if not inspection and not any(x in args for x in ("-c", "-S", "-E", "-M", "-MM")):
    args += ast.literal_eval(config["built-in options"]["c_link_args"])
os.execvp(cc[0], [*cc, *flags, *args])

