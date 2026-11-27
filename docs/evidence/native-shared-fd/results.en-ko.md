# JNOX #444 fresh extraction results · 새 디렉터리 재생 결과

Source revision `a1876bd4dc51e292fe63b0785bc4ea586b367b97`. The 6,243,488-byte archive has SHA-256 `6c48d4496ab89ece471ae1c9db21a039a57cea603916761d50655bc81b0ddb7f`. The independent QA runner reuses the original semantic decoder and exercises nine rehashed changes.

| Evidence / 근거 | Fresh extraction / 재생 결과 |
|---|---|
| FD cold-1 and cold-2 | 6 complete RAM snapshots per boot, all 12 original SHA-256 digests matched |
| Restored physical RAM / 복원한 물리 메모리 | 1,610,612,736 bytes from 105 distinct nonzero 64 KiB blocks and zero-filled ranges |
| Primary and QA readers / 관측기와 QA | Both stdout files reproduced byte for byte. The QA runner read the originals twice and rejected all nine changes |
| UART process exits / 프로세스 종료 | Slots 0, 1, 2, 3 exited 70, 71, 72, 0 in both FD boots and the window regression |
| Fixed-event prefixes dropped / 누락 행 | FD cold-1 1,255, FD cold-2 1,255, window 1,513 |
| Window cold boot / 창 데모 | Four scenes, eight external PPMs and four PNGs matched the full-frame fixtures. Logged PL011 input was `xnxq` |

두 부팅에서 캡처한 물리 메모리를 전부 복원해 원본 해시와 대조했다. QA는 파일 해시를 다시 계산한 아홉 조작 사본을 각각 의도한 이유로 거부했다. 별도 창 회귀에서는 화면과 입력 순서 및 네 프로세스의 종료값이 일치했다. 원본 자료를 오프라인으로 재생했으며, 실장 보드·GPU DMA·별도 Rust 링크 예제의 런타임은 여기서 검증하지 않았다.

The machine-readable evidence is in `fresh-replay.json`. Run `python3 -B replay.py` from the extracted archive.
