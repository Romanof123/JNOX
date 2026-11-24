# #281 first model observations / 최초 모델 관측

Commit `84214dd36094b4b3b489ab0232e9aa2f4774260b` on accepted main `9207337f95edfd7365a3cabf39b03d58607181f6`. Frozen directed contract and MG4 wire supplement at `5428cbe05ef2afb95252762a7ba1367b55bbda1d`. Six release images built serially with rustc 1.95.0 and both ordinals frozen before boot. QEMU SHA-256 `3ae2c079d4491362fece2abb641f0ff17d88e9008c9fcd13600f249b9bd1c862`. See build.log, freeze-digests, diagnostic-sha, frozen/*/freeze.json for full provenance. Twelve scored firsts followed the contract order and both diagnostic firsts followed. All fourteen launched exactly once.

| Workload | Selector | First 1 | First 2 | M destination |
|---|---|---|---|---|
| sum-65536 | global, partitioned, clustered | 0x1d each | 0x1d each | unobserved |
| CRC | global | 0x82cb0 | 0x82cb0 | CPU1 |
| CRC | partitioned | 0x82cb4 | 0x82cb4 | CPU0 |
| CRC | clustered | 0x82d40 | 0x82d40 | CPU0 |

Short 0/6 qualifying. Six unflushed UART journals leave both typed edges, installed frames, restore and subsequent work open. Stream 6/6 strictly inside the linked loop, including two real remote M returns and four same-core M returns. All six show both typed publication/consumption pairs. CPU1 retired opening P1 lease 2 and addressed CPU0 M lease 1. Reverse selected CPU1 lease 4 on all routes. Global selected M, installation v2 IRQ-saved. Partitioned/clustered selected replacement P1, installation v2 fresh even at its reused address. Five lease-bound TLS save/live pairs and four independent workload summaries checked per complete boot; raw CPU-specific exception files match IRQ and destination ERET. No physical board was exercised.

| Diagnostic | Terminal RAM | Independent observations |
|---|---|---|
| D1 short clustered | opening Pending, reverse Empty | 30 retained rows, opening publication only, initial M/P0/P1 fresh v1, P0 claim unobserved, early 0x1d |
| D2 stream global | opening Consumed, reverse Consumed | 72 retained rows, M saved v2 and P1 fresh v2, CPU0 IRQ and CPU1 ERET 0x82cf0, subsequent M work |

D2 PC equals the exclusive upper loop bound; its real remote return does not qualify as an inner-loop return. The receipts bind files and phases, and check that per-CPU exception files exist. Independent exception parsing provides the exact PC evidence. fixture.py reconstructed the normal finite-state 256 bytes from actual journal rows and initial stack bounds and compared them byte for byte in both positions. Quiescent terminal bytes cannot establish earlier transitions alone.

Pure Rust predecessor-preservation controls 5/5. Python handoff and diagnostic controls 6/6. Downgraded MG4 marker rejected offline. Current reader reopened 6 MG1, 6 MG2, 12 MG3 and 6 archived CRC sessions from the four public ZIPs enumerated by controls/history.py. Authenticated outcomes and individual partial reasons appear in controls/*.log. Direct generic QEMU SMP emitted the full expected transcript. Direct SW1 bounded UART/exception observation matched CPU-tagged PCs. Its first offline reader lacked pyelftools, so the corrected reader reopened the same session without a replacement boot. All-features build and invalid selector checks are retained.

## 한국어

짧은 합산 작업의 최초 6회 모두 M이 IRQ보다 먼저 종료되어 0x1d를 기록했다. 채점 UART에는 저널 행이 없어 인계 슬롯의 상태와 후속 작업을 확정할 수 없다. CRC 최초 6회 모두 엄격한 내부 루프 PC 조건을 충족했다. global 2회는 M이 CPU1에서 복귀했고 partitioned/clustered 4회는 CPU0에서 복귀했다. 모든 CRC에서 두 방향의 게시·소비, 실제 리스와 프레임 버전, CPU별 IRQ/ERET, TLS 및 후속 작업을 독립 확인했다.

별도 D1은 첫 슬롯 Pending, 역방향 Empty였다. D2는 두 슬롯 모두 Consumed였고 실제 원격 복귀 PC 0x82cf0은 루프의 제외되는 끝 주소였다. 두 진단의 실제 256바이트와 정상 전이로 계산한 바이트가 일치했다. 두 진단은 채점 12회에 포함하지 않았다. 물리 보드 동작은 미검증이다. 호스트 전이 5개 및 판독기 6개 테스트가 통과했다. SW1 최초 판독은 의존성 누락으로 실패했고 원본 그대로 재판독했다.

## Offline reproduction

Extract the ZIP, run `python3 restore.py` to materialize duplicate immutable binaries, and use the exact public source commit in `src/` for the public readers. The archive includes the independent scripts and all raw observations. From the extracted directory run `python3 independent.py`, `python3 fixture.py`, and `python3 diag-independent.py`. Reopen scored originals with `bash reopen.sh` and adjust PYTHONPATH to your pyelftools installation. controls/history.py uses the four archives from the exact source. Build and capture scripts are provenance; a repeat boot is a new observation.

[Original typed handoff runs and separate terminal observations / 정상 인계 최초 실행과 별도 터미널 관측](pi4-typed-handoff-first.zip) · SHA-256 56a8acd42144b6301427837596ab4fdeeeb50f8c94dc2ce614805efa86c579b8.
