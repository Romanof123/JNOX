# JNOX #275 independent first verification · 독립 최초 검증

Candidate `6f2f903623031fee8d81a4413569a8dd74ca0bd4`. Prospective contract `67559ab848e46aa3492ed8646481bf5a7847893d`. All three profiles were built serially on WSL Ubuntu ext4 using release/locked and -j1. Plain FIFO preceded traced FIFO on one frozen ELF (SHA-256 aeb2d339bf92c9142bfd0e7f209ef0a703294558da9c683ba109e65e3fd97e76). Both boots completed with terminal quiet and zero process exit.

| First position / 최초 위치 | Independent result / 독립 결과 |
|---|---|
| FIFO plain, A→B / B→A | PASS / PASS |
| FIFO traced, A→B / B→A | PASS / PASS |
| ordinary-pi plain / traced | QUALIFIED / QUALIFIED |
| ordinary-ceiling plain / traced | QUALIFIED / QUALIFIED |

Both workers blocked on the owner. Their priorities stayed at 7, the owner rose from 2 to 7 on first block, and dropped to 2 when it handed off to the first arrival. The second waiter remained blocked. The second grant preceded its actual dispatch. The worker acquire SVC at 0x40202474 resumed at 0x40202478 with x0 zero on the correct private frame. The linked worker calls acquire, tests x0, loads protected count/value, computes and stores both, and reports its observed previous values. For A then B the journal was 4→41→412, for B then A it was 4→42→421. Both counts advanced 0→1→2. Both ended with done mask 7, idle-only runnable mask 8, empty wait mask, free X and base priorities. Each boot provided 30 transitions and 30 stack-bounded restores. The independent checker matched all 30 ordered external returns and 28 SVC ELRs in the traced log to the ELF. The three constructed alterations were rejected separately on both original transcripts. All eight host mutex tests passed, including both fixed-ID arrival orders. Both invalid build configurations exited 101 before launch. The historical control qualifier scored all four first positions using original protected-work and compare-window rules.

동일 ELF로 일반 부팅을 먼저 수행한 뒤 외부 예외 추적 부팅을 수행했다. 각 부팅에서 두 대기자는 소유자가 X를 보유한 동안 차단됐고, 첫 차단 시 소유자의 유효 우선순위가 2에서 7로 상승했다. 최초 도착자에게 양도한 시점에 2로 복원됐으며 둘째 대기자는 차단을 유지했다. 두 작업의 저장 프레임·복귀 주소와 획득 성공값 0을 대조했다. 실행된 작업의 보호 값은 두 순서에서 각각 4→41→412와 4→42→421이었고 개수는 모두 0→1→2였다. 최종 상태에서 작업 세 개가 종료되고 락과 대기 집합이 비었다. 추적 부팅의 복귀 30개와 SVC ELR 28개를 ELF와 대조했다. 고정 ID 양도, 조기 복귀, 중간값 변조는 양쪽 기록에서 모두 거부됐다. 호스트 뮤텍스 시험 8개가 통과했고 두 잘못된 구성은 실행 전 거부됐다. 기존 PI 및 천장 우선순위 대조의 일반·추적 최초 위치 네 개 모두 원래 기준을 충족했다.

The verifier corrected a host checker assumption about global sequence numbering after capture. All six guest firsts were preserved. This finding covers the emulator cohort only. 실측 보드 차단 시간 및 제품 승인은 별도 범위다.

[Independent FIFO first runs and control evidence / 독립 FIFO 최초 실행과 대조 원본](ordinary-mutex-fifo-first.zip) · SHA-256 f1af2b285ec5d250b3e1d84bd972f1833e026d4a3129ee7ebcbd7fb9682be243.
