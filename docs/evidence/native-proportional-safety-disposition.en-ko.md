# Native proportional scheduling · Functional Safety disposition / 기능안전 판정

Reviewer role: Functional Safety, independent disposition of retained uncertainty under [#79](https://github.com/Romanof123/JNOX/issues/79) and [#337](https://github.com/Romanof123/JNOX/issues/337). Candidate f107b60fc1cb5bc83767ee98299d7042a3103b7a. Contract 29a4785070c64c5698fc1b44445fdcd7d7812da2.

**HOLD for unconditional engineering acceptance.** Accept the reported finite observations for the seven first launches on single Cortex-A72 QEMU virt, GICv2, TCG, 62.5 MHz counter, quantum 625000. Functional Safety reviewed the frozen source, original historical disposition, independent report and score summary, and verified the report and packet hashes. No additional guest or build was run in this review.

1. Preserve the original #79 failed verdict and EVD-021 thresholds. Four B/A/B/A IRQs, three completed returns and a shared failure marker bound the interval at epoch 0 slot 3. Identical loadable bytes and eight subsequent passes leave the failing branch and operands unknown. [Independent historical recovery](historical-scheduler-first-peer.en-ko.md)
2. For the six policy guests, journals through exit had zero drops. The independent verifier correlated timer interrupts, frame/return PC, private roots, stacks and products. Saved SP and PSTATE are guest witnesses. External exceptions corroborate ELR and return PC. Service brackets include kernel execution. Both dynamic guests yielded and slept three times. Peer PCs and final CRC establish productive execution; there is no per-wake RAM progress measurement.
3. At weights 1/2/4 the two all-ready seeds met every one of the 25 width-24 windows over 48 full closures. Minima were 5/3/11 and 1/7/10. Occupancy counts and measured service have separate records. Stable stride passed 48 prefix checks. Dynamic stride covered unequal leases, positive subquantum service, wake placement and normalized passes. The separate 6/3/11 seed-1 admission was rejected before dispatch at both interfaces, with owner A observed 5 against 6. Retain #54/#200 historical controls within their original scope.
4. **Outstanding condition.** In the native proportional runtime, timer guards 11–14 separately identify IRQ ID, CVAL, entry lower bound and control, with operands. Sync guards 100–109 carry condition IDs. The ordinary firsts produced no runtime fault. Guard 17 prints src and elapsed ticks when stop_elapsed returns false. Its wrong-owner predicate compares the policy's running owner with src, and its checked-add refusal would compare a pass and increment with an arithmetic limit. The emitted fields omit the running owner and the arithmetic pair. Prior to normal finish, in-memory journals are insufficient evidence of a preserved first-failure record. The admitted arithmetic bound explains why the sum cannot overflow in normal execution; the wrong-owner host control exercises the policy rejection without establishing the native diagnostic. Guard 24 also combines an out-of-range owner and a CURRENT mismatch. A general claim of recoverable first-condition operands remains open. Preserve a distinct diagnostic for each reachable cause, actual comparator operands, and counter/register/frame context before parking. Independently demonstrate recovery on a separately identified control while preserving ordinary first positions. Any new natural failure takes precedence for its own run.
5. Scope ends with these ELF identities, stated settings and captures. The successful runs and separate controls cannot identify #79's branch. Target draw source, workload, interference and service windows need deployment evidence. Customer hazard acceptance requires the authorized product safety owner. #79 remains open.

**한국어 판정 — 무조건부 엔지니어링 수용 보류.** 위 후보의 QEMU virt 단일 Cortex-A72, GICv2, TCG, 62.5 MHz, 양자 625000 조건에서 최초 실행 7건의 유한 관측 결과를 수용한다. 기능안전 검토자는 동결 소스, 과거 판정, 독립 보고서와 집계를 확인하고 두 파일의 해시를 대조했다. 빌드와 게스트는 재실행하지 않았다.

정책 실행 6건의 저널은 종료까지 유실이 없었다. 외부 예외 추적으로 복귀 PC를 대조했고 SP와 PSTATE는 게스트 저장 프레임 관측이다. 계측한 서비스에는 커널 시간이 포함된다. 두 시드의 48회 배정, 24회 폭의 25개 창, 스트라이드 48개 접두 구간, 실제 수면·재진입·부분 서비스 및 두 경로의 사전 거부 결과를 해당 조건에서 인정한다. 수면 중 상대 작업 실행은 PC와 최종 CRC가 뒷받침하며 매 기상 시점의 RAM 진행량은 계측하지 않았다.

타이머 가드 11–14는 조건과 피연산자를 구분한다. 그러나 가드 17은 호출자 슬롯과 경과 틱만 출력한다. 소유자 불일치 시 실제 정책 소유자가 누락되고, 산술 거부 시 패스와 증가량이 누락된다. 허용된 배정에서 정규화와 수치 상한으로 덧셈 오버플로를 배제하더라도 소유자 불일치의 진단 공백은 남는다. 가드 24도 소유자 범위 오류와 CURRENT 불일치를 함께 처리한다. 도달 가능한 거부 원인별 식별자, 실제 비교 피연산자, 레지스터·프레임 문맥을 정지 전에 보존하고 별도 대조에서 독립 복구해야 한다. 최초 정상 실행 기록과 자연 실패의 우선순위를 보존한다.

#79의 원래 B/A/B/A 인터럽트 4회와 복귀 3회로 epoch 0 슬롯 3 부근까지 특정했으나 분기·피연산자는 미상이다. 기존 실패 판정, EVD-021 기준 및 #79 미해결 상태를 유지한다. 실제 제품의 추첨원·간섭·서비스 창은 대상 환경 근거가 필요하며 고객 위험 수용은 권한 있는 제품 안전 담당자의 결정이다.

## Evidence / 근거

Original evidence directory: D:/codex/.jnox-work/338-first-verification-f107/

| File | SHA-256 |
|---|---|
| first-report.en-ko.md | 88fd696f6182aeff2a44ed889555e52e2833197c2523aff5ad9f46d6b4b7fa83 |
| native-proportional-first-f107.tar.gz (160146075 bytes) | e2eb4ce8cea2693deb8d265bb0ebc3e7447058cd9cbe59da9863c523801d6fae |
