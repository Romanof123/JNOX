"""Extract only simple regular files into a new directory. Python 3.10+."""
import argparse
from pathlib import Path, PurePosixPath
import shutil
import tarfile

parser = argparse.ArgumentParser()
parser.add_argument("archive")
parser.add_argument("destination", type=Path)
args = parser.parse_args()
args.destination.mkdir()
with tarfile.open(args.archive, "r:xz") as archive:
    for member in archive:
        path = PurePosixPath(member.name)
        if not member.isfile() or path.is_absolute() or ".." in path.parts or "\\" in member.name or ":" in member.name:
            raise ValueError(member.name)
        target = args.destination.joinpath(*path.parts)
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as output, archive.extractfile(member) as source:
            shutil.copyfileobj(source, output)
