# Native proportional first capture / 네이티브 비례 스케줄링 최초 원본

[Independent results / 독립 판정](native-proportional-applications-first.en-ko.md) · [Functional Safety disposition / 기능안전 판정](native-proportional-safety-disposition.en-ko.md) · [Immutable part manifest / 원본 목록](https://github.com/Romanof123/JNOX/blob/9e52dd911a6f0f1964bab166adab9cd92465c042/docs/evidence/native-proportional-f107-packet/manifest.json)

The sealed archive contains 160,146,075 bytes. Its SHA-256 is `e2eb4ce8cea2693deb8d265bb0ebc3e7447058cd9cbe59da9863c523801d6fae`. Run this Python in an empty directory. It verifies all 13 parts and the assembled original before making the archive available at its final name.

빈 디렉터리에서 다음 코드를 실행하면 13개 조각의 크기와 해시를 확인하고 검증팀이 봉인한 파일을 복원한다. 실제 부팅에 사용한 소스는 [검증 커밋](https://github.com/Romanof123/JNOX/commit/f107b60fc1cb5bc83767ee98299d7042a3103b7a)에 있다.

~~~python
import hashlib
import json
import tempfile
import urllib.request
from pathlib import Path

base = "https://raw.githubusercontent.com/Romanof123/JNOX/9e52dd911a6f0f1964bab166adab9cd92465c042/docs/evidence/native-proportional-f107-packet/"
expected = "e2eb4ce8cea2693deb8d265bb0ebc3e7447058cd9cbe59da9863c523801d6fae"
with urllib.request.urlopen(base + "manifest.json") as response:
    manifest = json.load(response)
target = Path("native-proportional-first-f107.tar.gz")
digest = hashlib.sha256()
with tempfile.TemporaryDirectory(dir=".") as tmp:
    staged = Path(tmp) / "original"
    with staged.open("wb") as out:
        for part in manifest["parts"]:
            with urllib.request.urlopen(base + part["file"]) as response:
                data = response.read()
            if len(data) != part["bytes"] or hashlib.sha256(data).hexdigest() != part["sha256"]:
                raise ValueError(part["file"])
            out.write(data)
            digest.update(data)
    if staged.stat().st_size != 160146075 or digest.hexdigest() != expected:
        raise ValueError("archive verification failed")
    target.hardlink_to(staged)
~~~
