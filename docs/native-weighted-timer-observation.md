# First timer-delivery observations / 타이머 전달 최초 관측

[Timing defect #184](https://github.com/Romanof123/JNOX/issues/184) · [Workload #177](https://github.com/Romanof123/JNOX/issues/177) · [Observed candidate](evidence/native-weighted-irq-second.en-ko.md) · [Measurement proposal](weighted-el0-timer-delivery-proposal.en.md)

Freeze one plain share/default boot and one separately scored instrumented share/default boot, in that order. Preserve the first session of each, including failure. Use the same #183 kernel and user binaries, source identity and 20-second watchdog; retain all CHG-088 timing and service limits. Capture the plain position with the original command. Add `--timer-observation` only for the second position. Replay each original directory against its externally retained session SHA-256 with `tools/native_weighted_replay.py`.

The instrumented lane targets /usr/bin/qemu-system-aarch64, Ubuntu 1:6.2+dfsg-2ubuntu6.31 (SHA-256 0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8). Built-in timestamped log events join timer-0 CVAL to the journal compare and PPI 30 to the matching lower-EL IRQ trace. Keep the original mixed log. Distinguish the timer's active-state recalculation record, GIC input, CPU IRQ output, and the guest handler's IAR read. The latter follows guest vector entry. The available event set does not timestamp entry to the timer callback or the exact instruction accepting the exception.

Report same-clock host intervals in microseconds with log quantization and sampling/clock limitations. Guest R, C and I retain counter ticks. Do not subtract these domains. QMP identifies the vCPU thread. Sample its Linux schedstat totals inside recorded host wall/monotonic intervals; queue delay is cumulative, not a timestamped deschedule. Missing detailed scheduling coverage or ambiguous events stay unobserved. No simulator timing measurement constitutes board timing.

## 한국어

#183에서 검증한 동일한 커널과 사용자 ELF를 써서 기본 클록 지분 작업을 두 차례 콜드 부팅한다. 첫 위치는 기존 명령이며, 둘째 위치만 `--timer-observation`을 사용한다. 20초 관측 한계와 CHG-088의 36개 실행 구간·누적 지분·종료 지분 기준을 두 위치 모두에 적용한다. 바이너리·관측 도구의 각 리비전, 해시, 인자와 최초 실패를 보존하고, 외부에서 고정한 세션 해시로 재생한다.

위에 지정한 Ubuntu QEMU 바이너리의 타이머 비교값, PPI 30 입력, CPU IRQ 출력, 타이머 재계산 기록과 게스트의 IAR 읽기를 원래 예외 로그에 대응시킨다. IAR는 게스트의 벡터 진입 뒤에 읽힌다. 타이머 콜백 진입과 인터럽트를 실제로 수락한 정확한 명령 시각은 관측할 수 없다.

호스트 사건 간 시간은 같은 시계의 마이크로초로, 게스트의 R·C·I는 카운터 틱으로 기록한다. QMP가 식별한 vCPU 스레드의 누적 실행·대기 통계에는 표본 경계의 불확실성이 있다. 개별 실행 중단 구간이나 원인을 분리할 수 없으면 미관측으로 남긴다. 보드 시간은 별도의 실측이 필요하다.
