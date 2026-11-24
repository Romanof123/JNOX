# MG2 opening P1 acquire in four clustered captures / 클러스터형 네 실행의 시작 P1 acquire

In the clustered captures in `docs/evidence/pi4-mg2-sparse-first.zip` and `docs/evidence/pi4-mg2-gic-first.zip`, use each terminal RAM dump's CPU1 tag-4 opening P1 frame pointer (row 3, word 2). At offsets 72 and 80 the saved X9 and X10 are 0xa9154 and 1 in both positions of both pairs. ELR is 0x82148. In the linked opening peer, 0x8210c loads the milestone into W10. A first-probe miss executes 0x82118 and replaces X10 with the error address 0xb08d0, which survives the loop and SVC. The four captures therefore record first-probe hits. The four-second deadline calculated before that probe served no wait in those captures.

두 묶음의 각 실행에서 CPU1의 시작 P1 트랩 프레임에 남은 X9, X10, ELR은 각각 0xa9154, 1, 0x82148이다. 첫 읽기가 실패하면 0x82118에서 X10이 오류 상태 주소로 바뀌므로 네 실행 모두 첫 읽기에 성공했다. 네 실행 모두 기한을 계산한 뒤 첫 읽기에서 대기를 마쳤다.

In the matched off/on pair, saved X8 minus four times the recorded 62,500,000 Hz gives the counter sample at 0x820f0. P1-local return-to-sample and sample-to-SVC-entry differences were 22,613 and 5,766 ticks off, and 24,596 and 3,970 on. In the stock GIC pair they were 16,971 and 3,758 off, and 17,038 and 3,904 on. These differences include surrounding instructions and model execution. None times the acquire instruction or the directed store.

No running-core invariant establishes the checked deadline addition before the counter wraps. Keep the fail-closed counter checks and the original four-second anchor even for first-probe hits. All four frames make a repeated timeout poll an inapplicable explanation for these runs. The next discriminator is inside the exception handler after SVC entry. The existing tag-3 stop is sampled at exception entry, so it cannot timestamp the policy retirement. A new treatment requires fresh linked addresses and all six original strict first positions.

X8에서 주파수의 네 배를 빼면 같은 CPU에서 대기 준비 직전에 읽은 카운터가 복원된다. 이 값은 acquire 명령과 SGIR 쓰기의 시각을 나타내지 않는다. 카운터 덧셈의 오버플로 가능성을 배제하는 실행 불변식이 확인되지 않아 기존 오류 검사와 네 초 기산점을 보존한다. 네 실행에서 반복 대기는 원인 후보에서 제외할 수 있다. SVC 진입 후 retire와 SGIR 쓰기까지의 구간은 열려 있다. 새 조건의 평가는 주소를 다시 확인한 여섯 최초 실행을 요구한다.

## Offline extraction / 오프라인 추출

Run this Python fragment from the repository root. The RAM base is 0x8d000 (577536); the journal symbol is 0xa9178, each per-CPU journal is 6944 bytes, and each committed event is nine little-endian words. The linked assembly is in each archive's disassembly.txt.

```python
import hashlib
import struct
import zipfile
from pathlib import Path

for archive, digest in (
    ("gic", "bc1e9c33329d76283ee6fef5f20154e06f7074e65e59379ceb690622505fb495"),
    ("sparse", "6448322726161ae31e1cf27b7945f27ca2048238c4bd784c6b8c9a3e98b1f468"),
):
    path = Path(f"docs/evidence/pi4-mg2-{archive}-first.zip")
    assert hashlib.sha256(path.read_bytes()).hexdigest() == digest
    with zipfile.ZipFile(path) as bundle:
        for name in sorted(n for n in bundle.namelist() if n.endswith("terminal-ram.bin")):
            ram = bundle.read(name)
            base = 577536
            journal = 0xa9178 - base + 6944
            def row(index):
                return struct.unpack_from("<9Q", ram, journal + 72 * index)
            used, lost = struct.unpack_from("<2Q", ram, journal + 6912)
            assert used > 6 and lost == 0 and row(0)[2] == 62_500_000
            trap, retired = row(3), row(6)
            assert trap[:2] == (0, 2) and trap[8] == 4 and trap[6] == 65535
            assert retired[:3] == (0, 2, 2) and retired[8] == 3
            frame = struct.unpack_from("<40Q", ram, trap[2] - base)
            assert frame[31] == trap[3] == 0x82148 and frame[39] == retired[4]
            sample = frame[8] - 4 * row(0)[2]
            print(name, hex(frame[9]), hex(frame[10]), sample - retired[3], retired[4] - sample)
```
[Independent review of four frames, instructions and bilingual release rows / 네 프레임·명령과 한영 출시 항목 독립 검토](scheduling-matrix-opening-peer-audit.en-ko.md).
