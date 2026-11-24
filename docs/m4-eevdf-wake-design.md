# POL-012 repeated wakes / 반복 기상 시험

CHG-036 · IMP-034 · planned EVD-034 (계획). Single vCPU, 128 consecutive one-quantum timer grants (CNTFRQ/100 ticks each). A and B stay runnable. A/B/C have weights 2/2/1 and one-unit requests. After each of its first 16 grants C returns to its worker and executes SVC #1. After 1, 2, 3, 4 intervening completed grants cyclically, C becomes runnable again. One scheduler instance retains request history across the full window.

단일 vCPU에서 타이머 할당 128회를 연속 관찰한다. A/B는 계속 실행 가능하며 가중치는 A/B/C 순서로 2/2/1이다. 매 할당은 CNTFRQ/100 카운터 틱이다. C는 처음 16회 실행 후 직접 SVC로 차단되고, 다른 태스크가 1·2·3·4회(순환) 실행하면 다시 준비된다. 정책 상태는 전체 구간에서 유지한다.

For every prefix and task, compare charged service with the sum of each turn’s runnable weight fraction. Require absolute error ≤20 quanta, 16 matched SVC → wake → task timestamps, maximum delay ≤6 intervening grants and ≤8 physical quanta. Report the full sampled delay distribution (nearest-rank p50/p95/p99 and max). These are finite fixture criteria. Product thresholds require owner approval.

각 접두 구간에서 실제 서비스와 매 턴 실행 가능 가중치 비율을 누적한 이상 서비스의 차이가 태스크별 20퀀텀 이하여야 한다. SVC·기상·태스크 재개를 16회 연결하고 최대 대기는 중간 할당 6회 및 물리 시간 8퀀텀 이하로 한다. 전체 표본과 최근접 순위 p50/p95/p99 및 최댓값을 보고한다. 제품 기준은 소유자가 별도 승인한다.

Raw QMP journal rows preserve runnable mask, virtual state, dispatched/restored and interrupted PCs, frame, progress and physical timer timestamps. Each wake links the SVC frame to its resume PC and first task-code counter sample. External capture retains the exception trace and ELF symbol addresses for independent reconstruction.

QMP 원자료에는 매 턴 준비 집합·가상 시간·복귀 및 인터럽트 PC·프레임·진행량·타이머 시각을 남긴다. 기상별 SVC 프레임과 재개 PC 및 태스크에서 읽은 최초 시각을 연결한다. 예외 추적과 ELF 심벌로 독립 재구성한다.

[Architecture](architecture.en.md) · [아키텍처](architecture.md) · [Verification plan](verification-plan.en.md) · [검증 계획](verification-plan.md)
