# Native REARM at 15 and 20 ms / 15 ms·20 ms 네이티브 재무장

**The normal supervisor accepted REARM at exactly 15 ms and refused it at exactly 20 ms, under both FIFO and RR on the engineering clock.** Each equality reproduced in three fresh boots of the same package. The independent verifier joined two committed, CRC-valid set permits to the protected evaluator, supervisor return, controller state and virtual ENABLE output. The 15 ms case produced an output rise with readback. The 20 ms case kept output low. All 24 guest boots had a proven inter-completion gap over 5 ms. Timing admission remains on hold.

**명령 수 기반 검증 시계에서 FIFO·RR 모두 정확히 15 ms의 재무장은 수락하고 20 ms의 재무장은 거부했다.** 정책별 각 경계에서 같은 패키지를 세 번 콜드 부팅해 재현했다. 실제 슈퍼바이저 판정, 정상 허가 두 건의 CRC와 커밋, 제어기 상태, 가상 ENABLE 출력을 원자료로 대조했다. 모든 게스트 실행에서 연속 완료 간격이 5 ms를 초과해 전체 시간 기준은 충족하지 못했다.

## Measured boundaries

All times refer to the older of the two accepted issue timestamps. The second issue was 10 ms later. The protected evaluator sampled a 62.5 MHz clock, with 16 ns per tick. Sender, controller and supervisor ran as ordinary processes on a single Cortex-A72 QEMU virt CPU, with their existing priorities 7, 6 and 4.

| Clock and policy | 15 ms evaluator offset | 20 ms evaluator offset | 15 ms / 20 ms |
|---|---:|---:|---|
| Default FIFO, first boot per case | +11,372 ticks | +13,667 ticks | accepted / refused |
| Default RR, first boot per case | +9,696 | +14,105 | accepted / refused |
| Engineering FIFO, first zero-lead captures | +236 | +239 | accepted / refused |
| Engineering RR, first zero-lead captures | +247 | +250 | accepted / refused |
| Engineering FIFO, selected leads 236 / 239 | 0 in 3/3 | 0 in 3/3 | accepted / refused |
| Engineering RR, selected leads 247 / 250 | 0 in 3/3 | 0 in 3/3 | accepted / refused |

Only the engineering launches used stock QEMU with `tcg,thread=single` and `-icount shift=0,align=off,sleep=off`. The first engineering observations supplied the four lead values. Each later launch used the ordinary package's numeric supervisor wake-lead argument. The exact and neighboring observations are instruction-counted virtual results. The default-clock observations landed after their nominal targets.

The 20 ms neighboring controls landed on D−1 and D+1 ticks under both policies. At D−1 the supervisor accepted the request, the controller found the first permit too old when it serviced the pending request, and ENABLE remained low. At D+1 the supervisor refused the request and ENABLE remained low. FIFO nominal lead arguments were 240 and 238 respectively, RR 251 and 249.

20 ms 경계의 양옆도 각 정책에서 실제 판정 시각을 확인했다. 한 틱 전에는 감독자가 수락했으나 제어기가 처리할 때 첫 허가가 이미 만료돼 출력이 계속 낮았다. 한 틱 후에는 감독자가 즉시 거부했다. 기본 클록에서 정확히 일치한 사례는 없었다.

## Timing and record coverage

| Clock | Boots | Consecutive CHECK gaps >5 ms | COMPLETE gaps definitely >5 ms | COMPLETE gaps straddling 5 ms | Largest COMPLETE gap bounds | Fixed scheduler rows dropped per boot |
|---|---:|---:|---:|---:|---:|---:|
| Default | 4 | 24/64 | 26/64 | 1/64 | 5.259616–5.262096 ms | 191–196 |
| Instruction-counted | 20 | 72/320 | 20/320 | 240/320 | 5.012464–5.013008 ms | 191–192 |

Each boot recorded 17 protected CHECK and 17 COMPLETE events. Safety-journal drops were zero. Each COMPLETE interval uses the protected entry and exit timestamps. For successive intervals [a,b] and [c,d] the true gap lies between c−b and d−a. The maximum release-to-COMPLETE interval was at most 2.808272 ms on the default clock and 0.022096 ms on the engineering clock. The fixed scheduler ring loses rows during these runs, which limits complete scheduler attribution. Captures describe virtual RAM outputs and do not establish physical actuation or a worst-case time bound.

보호 이벤트 로그 손실은 0이었다. 별도 스케줄러 링에서 실행당 191~196개 행이 손실되어 전체 디스패치 경로를 재구성하는 데 한계가 있다. 연속 완료 간격, 예약된 해제 시각부터 완료까지의 응답 시간, 재무장 경계의 판정은 각각 별도로 기록했다.

## Provenance and replay / 출처 및 재판독

Attempt 001 used source `6fbb5033c4f110b7a9941e4a74a1ea82e4e94fff`, public twin `732a7368cdeccfe520ceaf43ec794f5648ef0cbe` and tree `8c052aa494698fce5d0b2b69e3f6eecf3b8fb3ca`. The Python packager accepted both new selectors, while the kernel build rule still admitted only phase/collision. The attempted FIFO build stopped before launching QEMU. Its logs, source, role ELFs and incomplete package are preserved.

All 24 real boots used `047184a4c13ecc3f9bc74c4f94a04658e2b3ccc6`, public twin `15ec61c5acca83a4db251eee37bc5d560ec5b360` and complete tree `5c8ec0db0e4d146d4e2f36c13fbc322e6cc87f08`. The correction changed the kernel build predicate. The 25-entry ledger includes the initial refusal, original zero-lead runs, calibrated first attempts, fresh repeats and four neighbors. No guest launch timed out. Review per-boot evaluator rows, RAM and launch hashes in `verification/ledger.json`, `runs/`, `packages/` and `ledger.tsv`.

원래 빌드는 QEMU 실행 전에 실패했고, 수정본의 모든 게스트 캡처는 제한 시간 안에 완료됐다. 검증 묶음의 `verification/REPORT.en-ko.md`에는 25개 시도별 판정이 있다. 각 캡처는 원본 UART, RAM, ELF와 부팅 명령 해시를 포함한다.

The host controls cover 98 Python CLI cases and 124 direct Rust build-script cases across the frozen revisions. Under each policy, the Python CLI accepted 14 argument configurations and refused 35. The corrected Rust guard accepted 14 and refused 44 per policy. Python acceptance reached an intercepted kernel-builder call. Rust acceptance emitted its configuration and generated schedule before the deliberately omitted primary ELF stopped the host-only control. The original and corrected guard executables and first real kernel build failure are retained in `host-controls/`.

호스트에서는 Python CLI 98개와 Rust 빌드 인자 124개 사례를 별도로 재판독했다. 인자 검사 통과와 실제 게스트 부팅은 각각의 증거에서 확인할 수 있다.

On x86-64 Linux with Python 3 and GNU AArch64 binutils, extract the archive and run from its root.

```sh
sha256sum --quiet -c SHA256SUMS
bash scripts/replay-guest.sh .
python3 -B host-controls/replay.py source/production.tar
```

[Raw captures and offline replay](native-permit-rearm-ticks.tar.xz) (SHA-256 `d83937a537964d345f3b90b4fd54744855f42b88d4c3abfabe84291f6fb7215d`).
