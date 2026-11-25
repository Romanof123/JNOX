#!/bin/sh
# Usage: build_pi4_native_smp_store_plugin.sh QEMU /absolute/path/qemu-plugin.h NEW_DIRECTORY
set -eu
exec python3 - "$0" "$@" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys

if len(sys.argv) != 5:
    sys.exit("usage: build_pi4_native_smp_store_plugin.sh QEMU MATCHING_HEADER NEW_DIRECTORY")
script, model, header, out = [Path(arg).resolve() for arg in sys.argv[1:]]
source = script.with_name("pi4_native_smp_store_plugin.c")
out.mkdir(mode=0o700)
def digest(path):
    h=hashlib.sha256()
    with path.open("rb") as src:
        for block in iter(lambda:src.read(1<<20), b""):
            h.update(block)
    return h.hexdigest()

qsha=digest(model)
hsha=digest(header)
model_id=re.search(r"Build ID: ([0-9a-f]+)",subprocess.check_output(["readelf","-n",model],text=True))
if qsha!="651f3f1b5bb6f388d29f4542f9a3d142f40ab78811807db955903edf81b7a211" or not model_id or model_id[1]!="f2717a7db511bde6a6b5d68032618c682ca8f56b":
    sys.exit("frozen 60475a0 model identity mismatch")
if header.name!="qemu-plugin.h" or hsha!="335d4e472067e914add598b60d1f7cf0dc029cd48f62f66656d6a88c54d8d9f9":
    sys.exit("matching API-7 header required")
cc=shutil.which(os.environ.get("CC","cc"))
if not cc:
    sys.exit("C compiler unavailable")
glib=shlex.split(subprocess.check_output(["pkg-config","--cflags","glib-2.0"],text=True))
so=out/"observer.so"
argv=[cc,"-std=c11","-O2","-fPIC","-shared","-Wall","-Wextra","-Werror",
      "-I",str(header.parent),*glib,str(source),"-o",str(so)]
receipt=dict(qemu_sha256=qsha,qemu_build_id=model_id[1],header_sha256=hsha,
             source_sha256=digest(source),compiler=subprocess.check_output([cc,"--version"],text=True),
             compiler_sha256=digest(Path(cc)),argv=argv)
with (out/"build.log").open("xb") as log:
    result=subprocess.run(argv,stdout=log,stderr=subprocess.STDOUT)
receipt["exit"]=result.returncode
if not result.returncode:
    receipt["plugin_sha256"]=digest(so)
with (out/"build.json").open("x") as f:
    json.dump(receipt,f,indent=2)
    f.write("\n")
sys.exit(result.returncode)
PY
