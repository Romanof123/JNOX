# Independent corrected startup cohort / 첫 실행권 수정 독립 검증

Candidate 547fef88cf4144707c9f474bc4e9c3144667a1da. Approved design fd6cc8590bacfbc60d4a6ed7b8d22f1274c8db88. All 14 frozen first positions reached their expected terminal outcome. Twelve admitted runs completed 132 grants, every private product and all 26 application exit statuses. Both 9/8 runs refused before an opening or any private work. No row overflow or first-position recapture. Guest Cortex-A72 virt/GICv2, QEMU TCG, one vCPU, 128 MiB, default clock. Captures accumulated on Linux ext4 until all terminal lines arrived.

최초 14회 중 수용된 12회는 전체 앱 종료까지 진행했다. 132개 실행권 모두 첫 EL0 예외 상한이 허용된 1 ms 이내였고 9/8 구성은 두 번 모두 첫 실행 전 범주 2로 거부됐다. 모든 앱의 사설 결과와 종료 코드를 독립 검산했다.

| Package / 구성 | First plain blocks by slot / 일반 | First traced blocks by slot / 추적 | Exit statuses / 종료 | Grants per run / 실행권 |
|---|---|---|---|---|
| Saturated CRC, histogram | 14503, 20942 | 10673, 13978 | 17,29 | 16 |
| Isolated CRC, histogram | 8206,12015 | 6416,8515 | 17,29 | 10 |
| Reversed histogram, CRC | 11629,8127 | 7987,6113 | 29,17 | 10 |
| Triple CRC, histogram, energy | 8096,12148,36675 | 6353,8568,21211 | 17,29,43 | 18 |
| Sleep/yield, short | 3,1 | 3,1 | 59,53 | 4 |
| Sleep/yield, histogram | 3,12121 | 3,8442 | 59,29 | 8 |
| 9/8 refusal | 0,0,0 | 0,0,0 | refused 2 | 0 |

Independent slot-by-slot arbitration, remaining allocation, idle and exit replay matched all 132 winners. Saturation alternated eight grants. Both isolated orders delivered 0,1,idle,idle twice. The legal triple launched PID 2 first, followed by 0,1,2. Byte counts, cumulative zlib CRC-32, 16 histogram bins and scalar sums matched terminal private RAM. Private monotonic progress appeared in eight separate bursts per saturated process, five per isolated process, and all short-worker steps. Consecutive legal-triple PID 2 grants merged in host progress sampling, so the private delta for every individual adjacent grant in the plain triple remains indeterminate. Every long-running process produced a positive total and finished normally.

세 서버는 2,0,1,2 순서로 시작했고 세 프로세스 모두 작업을 마쳤다. 포화·격리와 단기 작업에서 각 프로세스의 독립적인 진행이 확인됐다. 세 서버 일반 실행에서 2번의 연속 실행권은 호스트 진행 샘플에서 한 구간으로 이어져 각각의 작업 증가량은 별도 확정하지 않았다.

Counter frequency 62,500,000 Hz, span 3,125,000 and guard 62,500. All twelve startups froze E at F+6,250 and recorded selection before F and verified L≥E. The first-launch [L−E,U−E] brackets, individual extrema and all sampled progress are supplied in scores.txt and scores.json. Across all grants maximum U−opening was 40,506 counts (0.648096 ms), maximum cutoff lateness 18,470 counts (0.29552 ms). Each measured outgoing stop occurred by nominal end. All grant and allocation owner-wall upper caps passed. Private PC, SP and frame address matched across 106/106 redispatches. The external IRQ/ERET log supplies raw PCs for the traced positions. Owner elapsed includes kernel residency. Actual per-instruction elapsed EL0 service was not measured.

첫 실행과 이후 실행권 전체에서 실제 진입 상한·컷오프 지연·슬롯 정지 및 할당별 소유 시간 상한을 충족했다. 사설 프레임 주소 및 PC/SP가 재개 지점과 일치했다. 타이머가 슬롯 끝보다 2 ms 넘게 늦은 자연 사건이나 첫 진입 전 억제는 이 수정 코호트에 발생하지 않았다.

The four short/wake runs retained q=1,d=8 at equality on slot 4, relinquished their tails, resumed the yield at slot 8 and completed three independent private reports. Other wake density branches and already-due sleep were unobserved on these guest inputs. Host policy tests passed 8/8, including stale owner, double debit, arithmetic overflow, post-exit wake and distinct density branches. Host-only threshold and absent-progress controls are in bounds.txt. CLI refused missing, duplicate, extra, conflicting, zero, above-period and malformed options. Raw Cargo accepted one valid setup and rejected all twelve negative configurations with their expected diagnostics. Supplemental overlarge reduced denominators refused category 3 on both guest boots with no opening or work. Physical target and admitted interference bounds remain open.

단기 앱은 4번 슬롯 등호에서 잔여 예산을 유지하고 8번 슬롯에서 양보 후 복귀했다. 다른 깨움 밀도 분기와 이미 만료된 수면은 게스트에서 미관측이다. 호스트 정책 8개 시험과 raw Cargo 정상 1개·거부 12개를 확인했으며 산술 범위 초과의 추가 게스트 두 실행 모두 범주 3으로 거부됐다.

Original c84539f plain/traced firsts have separate identities in original/. Its saturated plain UART ended inside a PID line, its saturated traced capture failed at transport, and both original triple positions inhibited. Those observations and all prior original outcomes are preserved in the enclosed earlier packets. The corrected cohort did not replace or rerun an original position. The 9/8 and arithmetic external exception logs are empty on pre-entry refusal. Timing qualification above concerns this virtual deployment.

이전 c84539f 최초 기록은 original/에 보존했다. 이전 포화 일반 기록의 마지막 상태 줄은 잘렸고 포화 추적 기록은 전송 오류로 중단됐다. 이전 세 서버의 두 실행 모두 진입 전에 억제됐다. 위 결과의 적용 대상은 해당 가상 배포 구성이다.