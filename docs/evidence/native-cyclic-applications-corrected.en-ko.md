# Independent cyclic workload / 독립 반복 시간표 검증

Source 213a8efa1d3dfa72eadc3ecb85fa4924b60ca3b8. WSL ext4, QEMU 6.2.0 (Ubuntu), one Cortex-A72 virt CPU, GICv2, 128 MiB, default TCG clock. Counter frequency independently observed in both ELFs was 62,500,000 Hz. The limit was 62,500 ticks. The first-position order below was frozen before boot. Each Windows tar was reopened and compared before the next boot.

Measured verdict / 실측 판정: 50/50 boundaries qualified, all four cold healthy boots completed. All five boots produced independently validated private EL0 results and exits 41/42, zero skipped edges, journal loss or application overflow. 각 창의 첫 복귀 표본과 실제 양의 작업량을 별도로 검사했다. 대기 창은 EL1 landing의 실제 counter read를 사용했다.

| First boot / 최초 부팅 | EL0 peak ticks | EL1 idle peak ticks | A blocks | B blocks | App records A/B |
|---|---:|---:|---:|---:|---|
|P-plain|43848|19637|973060|133580|2495/1241|
|Q-plain|47016|24344|792387|199913|1999/1738|
|P-traced|45927|15636|986055|142543|2496/1242|
|Q-traced|43521|22895|781543|197921|1997/1737|
|sleep-plain|control|control|994214|112734|2496/996|

## Per-edge arrival / 경계별 실제 도착

Each item gives sequence, selected context and actual x−d in ticks. score.json contains absolute d, i, r, x, j and block progress for each interval. Last listed edge closes the first window in frame four. A and B execute at EL0. I denotes EL1 idle.

- P-plain — 0 A 43848, 1 B 41587, 2 I 13554, 3 A 14251, 4 B 17676, 5 I 19637, 6 A 16520, 7 B 17504, 8 I 13201, 9 A 25654, 10 B 19030
- Q-plain — 0 B 47016, 1 I 24344, 2 A 26711, 3 B 17026, 4 B 16626, 5 I 9266, 6 A 14413, 7 B 17444, 8 B 20872, 9 I 10369, 10 A 10793, 11 B 14387, 12 B 16519, 13 I 9951
- P-traced — 0 A 45927, 1 B 45693, 2 I 15636, 3 A 18524, 4 B 15659, 5 I 9361, 6 A 17063, 7 B 16064, 8 I 10469, 9 A 13296, 10 B 17316
- Q-traced — 0 B 43521, 1 I 22895, 2 A 27330, 3 B 20057, 4 B 13892, 5 I 10362, 6 A 14808, 7 B 17145, 8 B 19823, 9 I 10870, 10 A 11659, 11 B 17125, 12 B 19957, 13 I 9976

## Products, context and sleep / 산출물·문맥·대기

Both address spaces were decoded independently through their actual TTBR0 translation tables. All records were checked against their per-block witness, elapsed sampling threshold, monotonic completed count, 900 ms interval and zero overflow. Terminal A result equals N × 5,559,680. B bins equal N × [1,8,28,56,70,56,28,8,1]. The absolute slot, ready mask, releases, spill, comparator, interrupted/return roots and saved PC/SP were replayed to termination. External traces independently matched the interrupted EL and PC and EL0 return PCs. SP and roots came from the journal and captured page tables. Q preserved the consecutive B windows across wraps and provided distinct samples and productive work in both.

- P-plain — A 5409902220800, B [133580, 1068640, 3740240, 7480480, 9350600, 7480480, 3740240, 1068640, 133580].
- Q-plain — A 4405418156160, B [199913, 1599304, 5597564, 11195128, 13993910, 11195128, 5597564, 1599304, 199913].
- P-traced — A 5482150262400, B [142543, 1140344, 3991204, 7982408, 9978010, 7982408, 3991204, 1140344, 142543].
- Q-traced — A 4345128986240, B [197921, 1583368, 5541788, 11083576, 13854470, 11083576, 5541788, 1583368, 197921].
- sleep-plain — A 5527511691520, B [112734, 901872, 3156552, 6313104, 7891380, 6313104, 3156552, 901872, 112734].

Sleep P / 수면 제어: B blocked at 12909089 ticks until 22888741. Its window tail entered idle, A ran in A’s reservation and remained selected when B became ready at 22896517. B resumed at 25351041. B’s requested absolute deadline was 362717504 ns and observation after waking was 402154192 ns. Both workers reached their private terminal products. B의 남은 창은 유휴였으며 A는 자신의 창에서 실행했다.

## Controls and limitations / 대조군과 범위

Two ordinary cooperative prime workers exited 12/12. All six invalid CLI tables were rejected (controls.log). The five corrected first positions ran once each. The A ELF matched across revisions. The rebuilt Rust B ELF had different symbol names in the ELF metadata. Its flattened load image was bit-identical across revisions (load-hashes). All five corrected positions used identical worker ELFs. An additional cooperative package with the counter-reading ELFs was built unused. The actual compatibility control used the established prime ELF.

## Corrected decision and natural syscall crossing / 수정된 결정 시각과 실제 경계 횡단

CONFIG 2 independently replayed exception entry [2] and selection counter [33]. Boot and IRQ samples matched. Every service decision was at or after entry. Absolute slot, expiry, wake, spill and eligible owner were evaluated at the new counter. The production Rust test syscall_crossing_an_edge_selects_at_completion_and_applies_due_wakes passed for exact-edge and post-edge decisions, testing an eligible peer or explicit idle with a simultaneous wake. Host command: rustc --edition=2021 --test tools/scheduling_tests.rs and run the named test.

One separate stock guest ran two ordinary SDK workers with alternating 1 ms windows. Each processed 220 independent batches of 8,192 pseudorandom bytes and called clock_gettime once after each batch. The first boot naturally crossed a boundary on the first A clock_gettime. Raw row 2 has i=6,715,751, d=6,726,872, decision=6,731,530 and B return r=6,747,447. The next exception entered at 6,752,619. The other 439 clock reads stayed in their entry windows. External log matched all 456 exception causes, ELs and PCs and all user return PCs. Both private batch counts were 220, both checksums 229,833,914, both exits 23. Journal loss was zero. This supplement was excluded from healthy timing.

별도 부팅에서 평범한 clock_gettime 처리 중 실제 경계를 통과했고, 완료 후 B로 복귀했다. 외부 예외 추적과 두 프로세스의 사설 메모리 계산 결과도 일치했다. 첫 코호트와 과거 기본 시계의 실패 기록은 별도로 보존했다. 수정 후보의 검증 범위는 지정된 QEMU 부팅 및 호스트 회귀 검사이다.

Files: score.json holds per-edge absolute values. runs/*.tar.gz contain UART, complete captured process spaces, launch argv, registers and external exception traces when enabled. Per-package manifests bind source and ELFs. The full original Git archive is retained separately on D:.
