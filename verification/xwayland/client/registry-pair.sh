#!/usr/bin/env bash
# Run in the checkout to be packaged. Output must be a new directory.
set -euo pipefail
work=$(realpath "${1:?installed upstream client work directory}")
runtime=$(realpath "${2:?original graphics runtime with Wayland OS adapter}")
inputs=$(realpath "${3:?pinned dependency cache}")
out=$(realpath -m "${4:?new output directory}")
mkdir "$out"
cd "$(git rev-parse --show-toplevel)"
export PYTHONDONTWRITEBYTECODE=1 CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true
tool=verification/xwayland/client
# Preserve the dependency builder's files. The registry relink needs its target prefix only.
mkdir "$out/link"
ln -s "$work/target" "$out/link/target"
sha256sum "$work/target/lib/libwayland-client.a" "$work/target/lib/libffi.a" \
    "$runtime/target/lib/libjnox-wayland-os.a" "$runtime/jnox.ini" >"$out/link-inputs.sha256"
# The previous four-global ELF used a CRT without envp. Recompile just the
# two graphics startup objects, keeping the archive builder's original flags.
python3 - "$runtime" "$out/runtime" "$PWD" <<'PY'
import ast, configparser, subprocess, sys
from pathlib import Path
original, dest, repo = map(Path, sys.argv[1:])
dest.mkdir()
config = configparser.ConfigParser(interpolation=None)
config.read(original/"jnox.ini")
opts = config["built-in options"]
cc = ast.literal_eval(config["binaries"]["c"])
flags = ast.literal_eval(opts["c_args"])
ld = ast.literal_eval(opts["c_link_args"])
sdk = repo/"sdk/native-aarch64"
flags[flags.index("-iquote") + 1] = str(sdk/"include")
for name, source in (("start.o","start.S"),("runtime.o","runtime.c")):
    index = ld.index(str(original/name))
    subprocess.run([*cc,*flags,"-Os","-c",sdk/"graphics"/source,"-o",dest/name],check=True)
    ld[index] = str(dest/name)
for i,arg in enumerate(ld):
    if arg.startswith("-Wl,-T,"):
        ld[i] = "-Wl,-T," + str(sdk/"graphics/app.ld")
opts["c_args"], opts["c_link_args"] = repr(flags), repr(ld)
with (dest/"jnox.ini").open("x") as fp:
    config.write(fp)
PY
runtime=$out/runtime
sha256sum "$runtime/start.o" "$runtime/runtime.o" "$runtime/jnox.ini" > "$out/startup.sha256"
for mode in default-display supplied-fd; do
    flags=(); elf=roundtrip.elf
    if [[ $mode = default-display ]]; then flags+=(--env-connect); elf=roundtrip-env.elf; fi
    python3 "$tool/build.py" registry "$inputs" "$runtime" "$out/link" "${flags[@]}"
    sh "$tool/registry-guest.sh" "$out/link/$elf" "$out/$mode" "${flags[@]}"
done
