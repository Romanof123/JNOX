"""Exercise the signal setup bodies from a prepared Xwayland 24.1.6 source tree."""
import argparse
from pathlib import Path
import subprocess
import tempfile


def function(source, signature):
    start = source.index(signature + "\n{")
    end = source.index("\n}\n", start) + 3
    return source[start:end]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("server", type=Path, help="patched server source directory")
    parser.add_argument("--cc", default="cc")
    args = parser.parse_args()
    utils = (args.server / "os/utils.c").read_text()
    connection = (args.server / "os/connection.c").read_text()
    signal = function(utils, "OsSigHandlerPtr\nOsSignal(int sig, OsSigHandlerPtr handler)")
    parent = function(connection, "static void\nInitParentProcess(void)")
    begin = connection.index("#if !defined(WIN32)", connection.index("if (ListenTransCount == 0 && !NoListenAll)"))
    end = connection.index("    ResetHosts(display);", begin)
    startup = "static void install_startup(void) {\n" + connection[begin:end] + "}\n"
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp)
        (out / "server.inc").write_text(signal + parent + startup)
        exe = out / "probe"
        subprocess.run([args.cc, "-std=c11", "-Wall", "-Wextra", "-Werror", "-I", out,
                        Path(__file__).with_name("probe.c"), "-o", exe], check=True)
        subprocess.run([exe], check=True)
