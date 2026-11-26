"""Prepare or verify the pinned Alpine arm64 virtio-net build headers offline."""
import argparse
import hashlib
import io
import os
from pathlib import Path
import stat
import sys
import tarfile
import tempfile
import zlib

APK_SHA256 = "f15c6785109c803a7d6cc2724020b288c6ab8237dbfc5d78501ea485f2b5b4d7"
HEADER_TREE_SHA256 = "901bb6ced9321e225b59395161f80a66146e3beaa9252f94c6430f545c84806b"
HEADER_FILE_COUNT = 9403
ARCHIVE_ROOT = "usr/src/linux-headers-6.18.65-0-virt/"

# These upstream aliases were excluded by the original regular-file intake.
# Match the archive records but never create, read through, or resolve links.
OMITTED_SYMLINKS = {
    "include/dt-bindings/clock/qcom,dispcc-sm8150.h": "qcom,dispcc-sm8250.h",
    "include/dt-bindings/clock/qcom,dispcc-sm8350.h": "qcom,dispcc-sm8250.h",
    "include/dt-bindings/clock/qcom,sm8650-dispcc.h": "qcom,sm8550-dispcc.h",
    "include/dt-bindings/input/linux-event-codes.h": "../../uapi/linux/input-event-codes.h",
}


def checked_path(name):
    parts = name.split("/")
    if any(p in ("", ".", "..") for p in parts) or "\\" in name or ":" in name:
        raise ValueError(f"unsafe archive path {name!r}")
    return parts


def verify_header_tree(root: Path) -> str:
    root = Path(root)
    if not stat.S_ISDIR(root.lstat().st_mode):
        raise ValueError(f"expected a regular directory at {root}")
    files = {}

    def fail_walk(error):
        raise error

    for dirname, dirs, names in os.walk(root, onerror=fail_walk, followlinks=False):
        directory = Path(dirname)
        for name in dirs:
            if not stat.S_ISDIR((directory / name).lstat().st_mode):
                raise ValueError(f"unexpected directory link at {directory / name}")
        for name in names:
            path = directory / name
            info = path.lstat()
            if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
                raise ValueError(f"expected an unlinked regular file at {path}")
            relative = path.relative_to(root).as_posix()
            files[relative] = hashlib.sha256(path.read_bytes()).digest()

    digest = hashlib.sha256()
    for relative, content_digest in sorted(files.items()):
        digest.update(relative.encode("utf-8") + b"\x00" + content_digest)
    actual = digest.hexdigest()
    if len(files) != HEADER_FILE_COUNT or actual != HEADER_TREE_SHA256:
        raise ValueError(f"build header tree mismatch: {len(files)} files, sha256 {actual}")
    return actual


def extract(archive, root):
    seen = set()
    for member in archive:
        name = member.name
        checked_path(name)
        if name in seen:
            raise ValueError(f"duplicate archive path {name!r}")
        seen.add(name)
        if not name.startswith(ARCHIVE_ROOT):
            continue
        rel = name[len(ARCHIVE_ROOT):]
        if rel != ".config" and not rel.startswith(("include/", "arch/arm64/include/")):
            continue
        if member.isdir():
            continue
        if member.issym() and OMITTED_SYMLINKS.get(rel) == member.linkname:
            continue
        if not member.isreg():
            raise ValueError(f"unexpected non-regular header {name!r}")
        target = root.joinpath(*checked_path(rel))
        target.parent.mkdir(parents=True, exist_ok=True)
        with archive.extractfile(member) as source, target.open("xb") as dest:
            dest.write(source.read())


def prepare(apk, output):
    data = apk.read_bytes()
    if hashlib.sha256(data).hexdigest() != APK_SHA256:
        raise ValueError("linux-virt-dev APK SHA-256 mismatch")
    # Alpine v2 packages have independently compressed signature, control,
    # and data members. Authenticate the full original bytes before inflating.
    for _ in range(3):
        stream = zlib.decompressobj(31)
        payload = stream.decompress(data)
        if not stream.eof:
            raise ValueError("incomplete APK gzip member")
        data = stream.unused_data
    if data:
        raise ValueError("unexpected trailing APK member")
    if os.path.lexists(output):
        raise ValueError(f"output already exists: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as tmp:
        staged = Path(tmp) / "headers"
        staged.mkdir()
        with tarfile.open(fileobj=io.BytesIO(payload), mode="r:") as archive:
            extract(archive, staged)
        digest = verify_header_tree(staged)
        staged.rename(output)
    return digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--apk", type=Path, help="original linux-virt-dev-6.18.65-r0.apk")
    group.add_argument("--verify-tree", type=Path, help="existing extracted header root")
    parser.add_argument("--out", type=Path, help="fresh header root on a case-sensitive filesystem")
    args = parser.parse_args()
    if (args.apk is None) != (args.out is None):
        parser.error("--apk and --out must be used together")
    try:
        digest = prepare(args.apk, args.out) if args.apk else verify_header_tree(args.verify_tree)
    except (ValueError, OSError, tarfile.TarError, zlib.error) as exc:
        sys.exit(str(exc))
    print(f"{digest}  {HEADER_FILE_COUNT} header files")


if __name__ == "__main__":
    main()
