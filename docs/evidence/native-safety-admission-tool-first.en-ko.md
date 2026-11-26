# Independent #389 host timing screen / 독립 호스트 타이밍 검증
Checker 7cafd1388e9bdef75d58b428dc79f4c771e9b254 directly follows fbbb5ceacaa39df47bac418ca9ef22390bc51b6b. The 24 FIFO/RR qualification archives in D:/codex/permit389-first-fbbb were extracted separately. For each, the exact frozen Python entrypoint ran without bounds and returned 1 (FAIL). Their archive digests are in crosscheck.json. No guest builds or boots were performed.

At 62.5 MHz the 1 ms phase ceiling is 62500 ticks. FIFO healthy A0 [116507,116640], A1 [88197,88243] exceeded. Deliberately overrun A4 was [91844,93070] FIFO and [91217,91320] RR. RR live A0 stage-3 lower bound was 159915; row 23 shows no actual block, so the upper bound is unknown. A later whole-segment failure does not establish an earlier decision miss. All 960 segment values were independently joined to their target, stage-3 and scheduler records. The selection for every new runnable mask agreed with the highest priority present, including idle for an empty mask.

Every original session member hash matched, the two kernel copies matched, launch and package manifests agreed, and the three UART boot hashes matched the separately supplied sender/controller/supervisor ELFs and the slot hashes. No justified worst-case bounds were supplied. Jitter, blocking, service, IRQ carry-in and endpoint uncertainty remain open. The checker correctly retained missing bounds and rejected an incorrect bounds kernel digest. A stale UART was rejected before scoring.

One integrity control failed. Adding a byte to a scratch copy of cap/user.elf while retaining the original receipt yielded byte-identical checker output. The normal checker echoes slot hashes from the manifest and does not verify the actual user ELF or three UART boot identities. The original packets independently validated above remain usable; checker-only per-slot integrity requires correction.

24개 원본 캡처에서 종료값은 모두 1이다. 960개 구간의 하한·상한, 실제 차단과 준비 마스크를 독립 대조했다. 실제 구간이 기준을 넘었고 최악시간의 분석적 입증 자료도 없다. 변형한 사용자 ELF가 검사기에서 탐지되지 않았다. 원본 패킷의 세 ELF와 UART 해시는 별도 검산에서 일치했다.

## Reproduction
Extract each original CASE.tgz in its own directory. Run python3 check.py CASE and retain stdout and exit status. Run python3 audit.py OUTPUT EXTRACTED_ROOT ORIGINAL_ARCHIVE_DIR to recompute the independent cross-check. Run python3 controls.py OUTPUT EXTRACTED_ROOT for the three disposable-copy controls.

[Raw captures and offline replay](native-safety-admission-tool-first.tar.gz) (SHA-256 `caae53761b568c72e2e18d6608f13d9413703f2ca8cf9717874d15ef76471250`).
