"""Verify original Alpine archives and reproduce the AArch64 Linux driver intake on Linux."""
import base64
import gzip
import hashlib
import io
import json
import lzma
import re
import struct
import subprocess
import tarfile
import tempfile
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent
ART = ROOT / "artifacts"
KEY = "alpine-devel@lists.alpinelinux.org-616ae350.rsa.pub"
ALPINE = "https://dl-cdn.alpinelinux.org/alpine/v3.23/main/aarch64/"
REV = "eb378b0757f36fcd639876eacb9315ef2cb9c075"
STABLE = "4700e3adf3b24b39dd4929e9c6d62811ba3ccea8"
REL = "6.18.65-0-virt"
MODULES = ["virtio_mmio", "failover", "net_failover", "virtio_net"]
PACKAGES = {
    "linux-virt": ("6.18.65-r0", "859ad147c29bf3c6d84a3ff84280aa5c94d890dc37ada31b072d099a8437f8e0"),
    "busybox-static": ("1.37.0-r30", "44c9abdfb970f398fa72c8382fe2d8808eea16beaf82daf6ac708b92f1b8659e"),
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run(*args, stdin=None):
    return subprocess.check_output(args, input=stdin).decode().strip()


def members(path, expected_count):
    data = path.read_bytes()
    result = []
    while data:
        stream = zlib.decompressobj(31)
        raw = stream.decompress(data)
        assert stream.eof
        end = len(data) - len(stream.unused_data)
        result.append((data[:end], tarfile.open(fileobj=io.BytesIO(raw))))
        data = stream.unused_data
    assert len(result) == expected_count
    return result


def verify_signature(parts, temp):
    key = ROOT / KEY
    assert digest(key.read_bytes()) == "d11f6b21c61b4274e182eb888883a8ba8acdbf820dcc7a6d82a7d9fc2fd2836d"
    sig_name = ".SIGN.RSA." + KEY
    assert parts[0][1].getnames() == [sig_name]
    sig = temp / "signature"
    sig.write_bytes(parts[0][1].extractfile(sig_name).read())
    assert run("openssl", "dgst", "-sha1", "-verify", key, "-signature", sig,
               stdin=parts[1][0]) == "Verified OK"


def materialize(out, name, data, source):
    (ART / name).write_bytes(data)
    out["artifacts"]["artifacts/" + name] = {"sha256": digest(data), "size": len(data), "source": source}


def driver_source(temp):
    recipes = (ROOT / "APKBUILD").read_text()
    sums = dict((name, value) for value, name in re.findall(r"^([0-9a-f]{128})  (\S+)$", recipes, re.M))
    checked = {}
    local_pat = re.search(r"source=\"https://cdn.kernel.org.*?\n\s*lts\.aarch64\.config", recipes, re.S)
    names = re.findall(r"^\s+(\S+\.patch)\s*$", local_pat.group(), re.M)
    for name in [*names, "patch-6.18.65.patch.xz"]:
        data = (ROOT / name).read_bytes()
        assert hashlib.sha512(data).hexdigest() == sums[name]
        checked[name] = digest(data)
        if name in names:
            assert not re.search(rb"(?m)^(diff |--- |\+\+\+ ).*drivers/net/virtio_net\.c(?:\s|$)", data)
    assert hashlib.sha512((ROOT / "config-virt.aarch64").read_bytes()).hexdigest() == sums["virt.aarch64.config"]
    # Reconstruct this one upstream source file with the stable patch named by the signed package's recipe.
    sourcefile = temp / "virtio_net.c"
    sourcefile.write_bytes((ROOT / "virtio_net.v6.18.c").read_bytes())
    chunks = []
    matched = False
    with lzma.open(ROOT / "patch-6.18.65.patch.xz", "rb") as delta:
        for line in delta:
            if line.startswith(b"diff --git "):
                matched = line.strip() == b"diff --git a/drivers/net/virtio_net.c b/drivers/net/virtio_net.c"
            if matched:
                chunks.append(line)
    assert chunks
    run("patch", "--batch", "--fuzz=0", str(sourcefile), stdin=b"".join(chunks))
    assert sourcefile.read_bytes() == (ROOT / "virtio_net.c").read_bytes()
    return {"aports_commit": REV, "kernel_stable_commit": STABLE, "tag": "v6.18.65",
            "source_path": "drivers/net/virtio_net.c", "file_sha256": digest(sourcefile.read_bytes()),
            "recipe_sha256": digest((ROOT / "APKBUILD").read_bytes()), "checked_patches_sha256": checked,
            "url": f"https://github.com/gregkh/linux/blob/{STABLE}/drivers/net/virtio_net.c",
            "recipe_url": f"https://gitlab.alpinelinux.org/alpine/aports/-/blob/{REV}/main/linux-lts/APKBUILD"}


def main():
    ART.mkdir(exist_ok=True)
    out = {"profile": "aarch64-qemu-virt-linux-candidate", "release": REL, "packages": {},
           "artifacts": {}, "modules": {},
           "trust": {"index_url": ALPINE + "APKINDEX.tar.gz", "key_url": "https://alpinelinux.org/keys/" + KEY,
                     "key_sha256": digest((ROOT / KEY).read_bytes()), "format": "APK v2 RSA/SHA1 control signatures and SHA256 datahash"},
           "scope": "Archive, config, ELF and source intake; guest execution, integration, device tree, fallback and restart require separate observations."}
    with tempfile.TemporaryDirectory() as td:
        temp = Path(td)
        index_file = ROOT / "APKINDEX.tar.gz"
        index = members(index_file, 2)
        verify_signature(index, temp)
        out["trust"]["index_sha256"] = digest(index_file.read_bytes())
        records = [dict(line.split(":", 1) for line in r.splitlines()) for r in
                   index[1][1].extractfile("APKINDEX").read().decode().strip().split("\n\n")]
        out["driver_source"] = driver_source(temp)
        for name, (version, hash_expected) in PACKAGES.items():
            filename = f"{name}-{version}.apk"
            path = ROOT / filename
            rows = [r for r in records if r.get("P") == name and r["V"] == version and r["A"] == "aarch64"]
            assert len(rows) == 1 and path.stat().st_size == int(rows[0]["S"])
            file_sha = digest(path.read_bytes())
            assert file_sha == hash_expected
            parts = members(path, 3)
            verify_signature(parts, temp)
            assert rows[0]["C"] == "Q1" + base64.b64encode(hashlib.sha1(parts[1][0]).digest()).decode()
            info = dict(line.split(" = ", 1) for line in parts[1][1].extractfile(".PKGINFO").read().decode().splitlines() if " = " in line)
            assert info["datahash"] == digest(parts[2][0])
            assert (info["arch"], info["pkgname"], info["pkgver"], info["commit"]) == ("aarch64", name, version, rows[0]["c"])
            out["packages"][filename] = {"url": ALPINE + filename, "size": path.stat().st_size,
                                         "sha256": file_sha, "aports_commit": info["commit"], "datahash": info["datahash"]}
            archive = parts[2][1]
            def extract(src, dst):
                data = archive.extractfile(src).read()
                materialize(out, dst, data, filename + ":" + src)
                return data
            if name == "busybox-static":
                extract("bin/busybox.static", "busybox.static")
                binary = ART / "busybox.static"
                assert "AArch64" in run("readelf", "-h", binary)
                assert "INTERP" not in run("readelf", "-l", binary)
                assert "NEEDED" not in run("readelf", "-d", binary)
                continue
            assert info["commit"] == REV
            config = extract("boot/config-" + REL, "config-" + REL).decode()
            out["config"] = dict(line.split("=", 1) for line in config.splitlines()
                                 if re.match(r"^CONFIG_(VIRTIO|VIRTIO_NET|VIRTIO_MMIO|ARM_GIC|SERIAL_AMBA_PL011|SERIAL_AMBA_PL011_CONSOLE|BLK_DEV_INITRD|RD_GZIP|MODULE_COMPRESS_GZIP|NET_NS|INET|DEVTMPFS|NET_FAILOVER|FAILOVER)=", line))
            assert out["config"]["CONFIG_VIRTIO_MMIO"] == out["config"]["CONFIG_VIRTIO_NET"] == "m"
            boot = extract("boot/vmlinuz-virt", "vmlinuz-virt")
            offset, size = struct.unpack_from("<II", boot, 8)
            assert boot[4:8] == b"zimg" and boot[24:28] == b"gzip" and offset + size <= len(boot)
            image = gzip.decompress(boot[offset:offset + size])
            assert image[56:60] == b"ARMd"
            materialize(out, "Image", image, f"vmlinuz-virt:EFI zboot gzip payload at {offset}, length {size}")
            prefix = f"lib/modules/{REL}/"
            deps = extract(prefix + "modules.dep", "modules.dep").decode().splitlines()
            for module in MODULES:
                paths = [m for m in archive.getnames() if m.startswith(prefix) and m.endswith("/" + module + ".ko.gz")]
                assert len(paths) == 1
                src = paths[0]
                packed = extract(src, module + ".ko.gz")
                data = gzip.decompress(packed)
                materialize(out, module + ".ko", data, module + ".ko.gz (lossless gzip decompression)")
                ko = ART / (module + ".ko")
                # Read the actual guest ELF section. Host kmod can resolve matching
                # built-in modules against the running WSL kernel despite a file path.
                strings = run("readelf", "--string-dump=.modinfo", ko)
                info_rows = re.findall(r"^\s*\[\s*[0-9a-f]+\]\s+(\w+)=(.*)$", strings, re.M)
                fields = {key: ",".join(v for k, v in info_rows if k == key).strip()
                          for key in ("name", "vermagic", "depends", "firmware", "license")}
                assert fields["name"] == module and fields["vermagic"].split()[0] == REL
                assert "AArch64" in run("readelf", "-h", ko)
                dep = next(line.split(":", 1)[1] for line in deps if line.split(":", 1)[0] == src[len(prefix):])
                fields["transitive_modules_dep"] = dep.split()
                out["modules"][module] = fields
    (ROOT / "provenance.json").write_text(json.dumps(out, indent=2, sort_keys=True) + "\n")
    for name in ["Image", "vmlinuz-virt", "virtio_net.ko.gz", "virtio_net.ko", "busybox.static"]:
        print(name, out["artifacts"]["artifacts/" + name]["sha256"])
    print("PASS", REL, out["modules"]["virtio_net"]["vermagic"], out["driver_source"]["file_sha256"])


if __name__ == "__main__":
    main()
