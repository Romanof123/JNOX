# Protected controller completion watchdog / 보호 제어기 완료 감시

CHG-312, [#392](https://github.com/Romanof123/JNOX/issues/392), REQ-019 and REQ-024. On the single-vCPU virt permit qualification profile only, select `--safety-watchdog`. The fixed scheduler programs its existing physical CNTP/PPI30 for the earliest scheduler, IPC or watchdog deadline. A silent CPU-bound user process or idle WFI therefore needs no cooperative polling. Preserve the qualification IRQ journal's raw IAR, CVAL, entry and return, plus the virtual register's independent atomic transition bracket. Report delayed delivery and its cause only when observed.

OPEN by the supervisor inhibits and creates an unprimed fresh generation. Only a successful controller COMPLETE (SVC60 or fused SVC63 command 3) advances its completion stamp. CHECK, loan traffic, information reads, invalid COMPLETE, sender progress and REARM cannot refresh it. ENABLE requires a primed live generation and all original fresh-pair/rearm conditions. With last completion L and frequency F, the first violating physical tick is D = L + floor(5F/1000) + 1. At each completion arbitrate the old D first; never revive a broken chain. At now >= D latch the watchdog fault, clear pending rearm, inhibit, and retain the due/observation and transition evidence. Ambiguous or overflowing counter arithmetic fails closed. On endpoint death stop the watchdog and inhibit. Only the already authorized fresh OPEN can recover a live session. Report INFO bit 4 for primed and bit 5 for a watchdog fault. Watchdog expiry does not alter the last independently accepted permit identity. Preserve the separate 20 ms permit validity check.

Acceptance includes an unprimed refusal, successful real completions and rearm, silent controller under uninterrupted EL0 work or idle, real PPI30 delivery, stale/duplicate/wrong-issuer commands, late COMPLETE, withdrawal and recovery in a new generation. Deterministic boundary arithmetic and observed live timing are separate. A late timer sample may show detection later than D; retain that delay and the output bracket. Watchdog enforcement does not qualify the independent 1 ms phase budget, establish WCET or authorize a physical device.

단일 CPU virt 보호 시험에서만 선택한다. 기존 물리 타이머 PPI30에 스케줄러·IPC·감시 기한의 최솟값을 설정하므로 사용자 호출이 없는 계산 중이나 WFI에서도 작동한다. 감독자의 새 OPEN은 출력을 차단하고 미준비 상태를 만든다. 제어기의 성공한 COMPLETE만 완료 시각을 갱신하며 CHECK·조회·대여·재무장은 갱신하지 않는다. 마지막 완료 L 이후 5 ms까지 허용하고 그다음 표현 가능한 틱 D부터 단절이다. 뒤늦은 완료보다 기존 기한 판정을 먼저 수행한다. 단절·시계 모호성·종료 시 허가를 차단하고 실제 비교값·IRQ 진입·판정 및 가상 장치 전환 구간을 보존한다. 살아 있는 채널은 승인된 다음 세대 OPEN으로만 복구한다. INFO의 비트 4는 준비, 비트 5는 감시 고장이다. 기존 20 ms 허가 만료와 새 세대의 복구 조건을 유지한다. 실측된 지연은 그대로 보고하며 1 ms 단계 승인·WCET·실물 장치 안전 판정은 별도다.

## Deterministic boundary check / 결정적 경계 검증

From the repository root on a Rust host, run the production arithmetic and transition unit directly:

```sh
rustc --edition=2024 --test kernel/src/completion_watchdog.rs -o /tmp/jnox-watchdog-boundaries
/tmp/jnox-watchdog-boundaries
```

The same module decides poll, prime, renewal and fault in EL1. Inputs in these vectors are private host-test values; the kernel still obtains real counter samples internally. The vectors exercise D−1, D, D+1, sticky late refusal, fractional periods, zero frequency rejection, unrepresentable deadlines and clock discontinuity. They give no evidence of IRQ delivery, caller identity, real cadence, register transitions or physical timing. [#392](https://github.com/Romanof123/JNOX/issues/392)

위 명령은 커널과 같은 결정 코드를 실행한다. 호스트 시험값을 게스트에 전달하는 인터페이스는 없다. 경계 전후, 늦은 갱신 거부, 주파수와 오버플로, 단조 시계 위반을 확인한다. 실제 IRQ·호출 주체·장치 전환·물리 시간은 별도 관측이 필요하다.
