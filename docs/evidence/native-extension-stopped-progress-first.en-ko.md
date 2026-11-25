# Native evaluator fail-stop first acquisition · 네이티브 평가기 고장 복구 최초 취득

**Confirmed native injected cancellation, watchdog expiry, delegated fallback and real idle on the named virtual Cortex-A72.** The revised public reader f5b9df23b8d8777fe6e2dbeff1b334e2ab959398 qualified both source-built cohorts from 2c4b0c227674e84ee488638cd8ab0c991ada64b7. All 22 frozen first boots completed. The operator scored all 22 without runtime issues. Independent UART, exception/register and RAM decoding also matched all 22. Direct hardware readback at the instant of timer arm and a direct dump of saved SPSR_EL1 remain unavailable. A verdict requiring these direct register observations remains incomplete. 제품군·실기 출시는 보류한다. 지정 가상 CPU에서 주입한 평가기 정지, 실제 인터럽트에 의한 취소, 임계값과 정확히 일치하는 워치독, 대체 작업·유휴 실행을 관측했다. 타이머 설정 순간의 하드웨어 재판독과 저장된 SPSR_EL1 자체의 직접 덤프는 미관측이다.

## Observed native recovery / 관측된 복구

| Captured active cell | Last progress / age | PPI30 after D (ticks) | Cutoff result | Returned work |
|---|---|---:|---|---|
| A 1 | k0 / 1 | 9,116 | Missing publication, heartbeat 1 | Slot 1 temperature, grant 1 |
| A 3 | k0 / 3 | 8,518 | Watchdog, heartbeat 1 | Slot 1 temperature, grant 1 |
| B 9 | k7 / 2 | 8,623 | Inclusive watchdog, heartbeat 6 | Actual EL1 idle, grant 0 |

At all three external IRQs the last running CPU block had PC 0x40206ee4, live PSTATE 0x60000345 (EL1h, I clear), private evaluator SP 0x40245110 and x0 equal to the frozen cell. Vector entry retained that SP. The subsequently observed handler SP was on the caller stack. The actual cutoff register read gave CVAL=D and CNTP_CTL low bits=5. The scheduled compare at arm and its later physical register readback agree. The QEMU exception identifies ELR at the self-branch. The pre-IRQ live PSTATE and vector state establish the interrupt transition; no direct saved SPSR_EL1 field was recorded. A k1 TTBR0/ASID was 0x3000040a8b000, A k3 and B k9 0x1000040261000. Independent page-table walks found the actual branch instruction in privileged executable memory and SP in kernel read/write execute-never memory.

The approved typed entry was observed with masked IRQs before unmask on its private stack. Across the active and dormant runs, one opening snapshot governed each cell. A produced 21012021 under injection and 22021011 dormant. B produced 2201101I0II0 in both, where I means actual EL1 idle. At B k7 the empty delegate set still completed evaluation and advanced heartbeat. Reserved alarms ran at k8, the k9 loop was abandoned, k10 resumed timely evaluation, and reserved alarms exited at k11. A k2 executed reserved voltage, and k4 resumed delegated nomination. Timely, missing and expired cases were distinguished by cause and publication even when owners matched. No skipped cells, missing journal rows or duplicate grants. No live obsolete publication attempt occurred.

## Ordinary and timing results / 일반 실행과 시간

All limits use F=62,500,000 ticks/s, Q=6,250,000. Useful-work bounds come from a real post-product syscall or actual idle landing. The original maximum was 3,125,000 ticks. The allowed end lateness was strictly below 1,562,500 ticks.

| Frozen firsts | Boots | Cells | Maximum useful-work upper bound, ticks | Operational ends | Maximum end lateness, ticks |
|---|---:|---:|---:|---:|---:|
| Six ordinary configurations | 12 | 102 | 1,773,806 | 90 | 25,495 |
| B ordinary | 2 | 24 | 1,612,193 | 22 | 14,731 |
| A/B dormant/active | 8 | 80 | 1,757,979 | 72 | 24,486 |

ELF-bound private RAM matched every declared count, batch sum, cumulative and signed exit. Voltage 6596/9900, exit 30; temperature 18/41/62, exit 31; alarms 65536/131072/196608, fourth 262144 where selected, exit 32. Terminal timer cancellation was separate. Independent verification walked each process root to its own stored result. Traced positions supplied exception and return evidence. Plain positions supplied terminal RAM and guest register observations. 최초 22회에서 앱 사설 결과, 종료 상태, 실제 유휴·복귀 시각과 각 운영 종료 한계를 확인했다. 관측 범위는 이 가상 장치와 이 인위적 고장 입력이다. 자연 고장 빈도·실기 지연·제품 승인은 미결이다.

## Provenance, controls and retained firsts / 출처·대조·최초 기록

The reader revision changed only its verifier and historical evidence. All predecessor receipt file hashes were checked before replay. A/B source-built preflights each passed against the original kernel, object blobs, actual compiler/linker argv and maps, complete independent standalone ordinary ELF and archived three-app identities. The original objects and binaries remain in native-extension-source-built-first-raw.tar.gz, SHA-256 797d1e8594eb7ea645895e075e4bc99f925ed3061415c131107aab02706a99ca. A accepted and B ordinary and fixtures reused the exact sealed cohort packages. The other five ordinary packages used the current checkout and same archived app bytes. Negative replays rejected a mistyped or duplicate erratum flag, executable-stack -z operand, foreign search directory and unknown option. All 194 scheduling host tests passed. The three exact shared transaction cases killed all 11 targeted mutations. The original linked normal and stopped diagnostic first plain/traced boots passed 4/4; the diagnostic matched 15 external IRQ PCs.

B plain’s first offline invocation omitted --plain, requested a missing exceptions.log and failed. The original capture, failing replay and corrected offline result are all retained. Correct replay mode passed on the identical session. No guest was relaunched. All ordinary receipts, flags and linked/host outcomes were sealed before starting the eight fixture firsts. Prior failed gates and prior engineering approvals remain separate. The source artifacts, fix-mode wrappers, exact launch and session records, per-run traces and memory, independent reader and per-cell outputs are included or bound to the predecessor archive. No hardware, natural fault or family release decision is implied.
