# #345 / #346 integrated native scheduling verification · 병합본 독립 검증

**The integrated commit passed the frozen transaction logic controls and the six ordinary native configurations on the named virtual Cortex-A72. Native missed publication, cutoff cancellation and inclusive watchdog expiry remain unobserved.** 병합본에서 실제 커널과 공유하는 거래 상태 전이 및 사전 고정한 가상 머신의 네이티브 앱 사례가 통과했다. 네이티브 미게시·평가 취소·감시 만료의 실시간 증거는 미관측이다.

Source 1cf56b1d08d0ea516994ea9e15e70aa11a3f1ad0, fresh detached ext4 checkout, LF tracked source, clean tracked status. Frozen boundaries at 016747d and native contract at 3aaaf88. The 12 original boots at 5c378fa remain separate. All integrated packages reuse this run’s three exact ELF hashes, including separately linked Rust temperature ELF 5a5e88cf7c66ec8cc65842220e03285a42976483925f8367a8357a13743e8814. Its whole-file hash differs from the previous checkout’s Rust ELF. identity, package manifests and launch/session digests bind actual binaries, rules, arguments, machine and build data. 이전 기록과 이번 바이너리를 구분했으며, 여섯 패키지는 동일한 세 앱 ELF를 사용했다.

## Production transaction and mutation controls / 공유 거래 로직

The native producer invokes begin, evaluates the actual policy with IRQs enabled, then masks IRQs for staged publish and its counter sample. The PPI consumer invokes open and consume; terminal cleanup invokes abandon. The native and host modules include the same kernel/src/native_extension_transaction.rs. The ordinary suite passed 194/194. The independent filtered command ran all three fully qualified tests, with input/output logs in cases. The mutation harness made one isolated source change at a time, compiled it, ran one exact test, checked that it failed, and restored the source. All eleven challenges were detected.

| Test under native_extension_transaction::tests | Covered decisions | Mutations detected |
|---|---|---|
| publication_and_absence_at_cutoff | cutoff−1 held until due, commit once, reject at cutoff and +1, absent reply with eligible delegate or idle | accept equality, early consume, duplicate consume, advance trusted heartbeat before commit |
| obsolete_producer_cannot_change_a_newer_transaction | stale producer with empty/staged newer cell, stale consumer, inactive producer, terminal cleanup | erase newer reply, ignore producer cell, ignore consumer cell, retain abandoned reply |
| missing_progress_reservations_and_inclusive_watchdog | cell 1 absent before expiry, reserved 2 served or idle, elapsed/exact expiry at 3, fresh progress, rejected regression | exclusive watchdog threshold, lost reset, accept regressed progress |

호스트 194개 중 거래 전용 3개를 정확한 이름과 입력으로 다시 판정했다. 독립 소스 복사본의 조건을 하나씩 변경한 11가지 변이에서 해당 테스트가 모두 실패했다. 호스트 판정은 시간·레지스터에 대한 게스트 실측과 구분한다.

## Integrated native firsts / 병합 네이티브 최초 기록

Six normal and six external exception-traced cold launches, one of each per named configuration, passed without retries. All 102 first-work bounds met 3,125,000 ticks (50 ms). Maximum observed upper bound was 1,763,614 ticks (28.217824 ms), using post-product SVC or actual idle landing. All 74 delegated publications were timely. All 90 operational PPI30 service-end IRQs met the original 25 ms tolerance, maximum lag 27,276 ticks (0.436416 ms). No skipped cells, lost journal rows or native evaluator cancellations. The stopped terminal compare belongs to final application exit.

The independent reader decoded UART and performed physical page-table walks against ELF symbols in terminal RAM. Voltage produced 6596/9900 and exit 30, temperature 18/41/62 and exit 31, alarms 65536/131072/196608 and exit 32. Extended alarms reached 262144. Each batch count, sum and cumulative matched. Owners were 22021011 for accepted, 11012022 for stale/CPU/fuel, 2202201131 for the extended case, and 220213110 with reserved sleep (3 denotes real idle). Wrong generation, out-of-mask CPU, five-step fuel fallback, yielded fallback, reserved exit cell 8 and absolute 400 ms sleep with cell 5 idle all matched. The integrated replay matched the external EL0 returns, exception PC/level/cause, journal PC/SP/root continuity and actual EL0 work upper bounds. All twelve firsts are included.

최초 일반·추적 부팅 12건에서 세 앱의 사설 메모리 결과와 종료값, 예약 유휴·기상, 위임 대체 경로를 확인했다. 통합본의 모든 위임 응답이 기한 내 게시되었다. 실제 해석 중 IRQ, 늦은 게시 시도, 네이티브 취소·감시 만료 및 기상 시각과 경계의 정확한 일치는 관측되지 않았다. 지연 주입이나 재시도는 없었다.

## Startup and compatibility / 시작 및 호환성

The default boot passed two cold launches. The real Cargo selector matrix passed, including startup/profile precedence and board/feature rejection. All 140 Python discovery tests passed. Package and raw Cargo controls rejected malformed programs, missing applications, mixed modes and invalid digests; one ordinary cooperative input explicitly rejected the extension option outside extension mode. The first outside-policy CLI probe rejected an invalid policy literal and is preserved separately. Linked normal and diagnostic plain/traced cold boots passed 4/4, with the original diagnostic’s nine-cell results and 15 IRQ PCs, including its own stopped evaluator. No native event is inferred from the linked case.

기본 부팅 두 번, 실제 Cargo 선택기, Python 140개, 원시 빌드·공개 CLI의 잘못된 입력, 연결형 기존 이미지 일반·추적 4건이 통과했다. 제품의 채택 규칙·부하·시간 한계·실기·위험 및 출시 승인은 제품 책임자에게 남아 있다. The archive includes the raw firsts, manifests, journals, snapshots, host case outputs, independent scorers and mutation code. Build caches, mutation checkout copies and host test executables are omitted.
