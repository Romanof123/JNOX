# Independent cyclic workload / 독립 반복 시간표 검증

Source c3c89fd04d37992bfd00912814e8171e255749c8. WSL ext4, QEMU 6.2.0 (Ubuntu), one Cortex-A72 virt CPU, GICv2, 128 MiB, default TCG clock. Counter frequency independently observed in both ELFs was 62,500,000 Hz. The limit was 62,500 ticks. The first-position order below was frozen before boot. Each Windows tar was reopened and compared before the next boot.

Measured verdict / 실측 판정: 50/50 boundaries qualified, all four cold healthy boots completed. All five boots produced independently validated private EL0 results and exits 41/42, zero skipped edges, journal loss or application overflow. 각 창의 첫 복귀 표본과 실제 양의 작업량을 별도로 검사했다. 대기 창은 EL1 landing의 실제 counter read를 사용했다.

| First boot / 최초 부팅 | EL0 peak ticks | EL1 idle peak ticks | A blocks | B blocks | App records A/B |
|---|---:|---:|---:|---:|---|
|P-plain|42113|10332|979662|142222|2498/1242|
|Q-plain|46087|21927|784729|199145|1998/1739|
|P-traced|45082|10996|987733|142722|2491/1241|
|Q-traced|45794|26896|784403|199694|1996/1738|
|sleep-plain|control|control|980971|114035|2497/997|

## Per-edge arrival / 경계별 실제 도착

Each item gives sequence, selected context and actual x−d in ticks. score.json contains absolute d, i, r, x, j and block progress for each interval. Last listed edge closes the first window in frame four. A and B execute at EL0. I denotes EL1 idle.

- P-plain — 0 A 42113, 1 B 40962, 2 I 9624, 3 A 12192, 4 B 14797, 5 I 10332, 6 A 10226, 7 B 14010, 8 I 9411, 9 A 11852, 10 B 12894
- Q-plain — 0 B 46087, 1 I 21927, 2 A 24636, 3 B 17636, 4 B 20543, 5 I 13009, 6 A 15387, 7 B 16466, 8 B 20193, 9 I 9741, 10 A 12926, 11 B 16350, 12 B 13511, 13 I 11903
- P-traced — 0 A 45082, 1 B 42480, 2 I 10996, 3 A 15036, 4 B 16184, 5 I 10828, 6 A 13906, 7 B 17161, 8 I 9727, 9 A 16444, 10 B 16486
- Q-traced — 0 B 45794, 1 I 26896, 2 A 27062, 3 B 18824, 4 B 10891, 5 I 8933, 6 A 11403, 7 B 15917, 8 B 19336, 9 I 10041, 10 A 12282, 11 B 14339, 12 B 11114, 13 I 9641

## Products, context and sleep / 산출물·문맥·대기

Both address spaces were decoded independently through their actual TTBR0 translation tables. All records were checked against their per-block witness, elapsed sampling threshold, monotonic completed count, 900 ms interval and zero overflow. Terminal A result equals N × 5,559,680. B bins equal N × [1,8,28,56,70,56,28,8,1]. The absolute slot, ready mask, releases, spill, comparator, interrupted/return roots and saved PC/SP were replayed to termination. External traces independently matched the interrupted EL and PC and EL0 return PCs. SP and roots came from the journal and captured page tables. Q preserved the consecutive B windows across wraps and provided distinct samples and productive work in both.

- P-plain — A 5446607228160, B [142222, 1137776, 3982216, 7964432, 9955540, 7964432, 3982216, 1137776, 142222].
- Q-plain — A 4362842126720, B [199145, 1593160, 5576060, 11152120, 13940150, 11152120, 5576060, 1593160, 199145].
- P-traced — A 5491479405440, B [142722, 1141776, 3996216, 7992432, 9990540, 7992432, 3996216, 1141776, 142722].
- Q-traced — A 4361029671040, B [199694, 1597552, 5591432, 11182864, 13978580, 11182864, 5591432, 1597552, 199694].
- sleep-plain — A 5453884849280, B [114035, 912280, 3192980, 6385960, 7982450, 6385960, 3192980, 912280, 114035].

Sleep P / 수면 제어: B blocked at 12913467 ticks until 22891189. Its window tail entered idle, A ran in A’s reservation and remained selected when B became ready at 22900173. B resumed at 25350958. B’s requested absolute deadline was 362836528 ns and observation after waking was 402248064 ns. Both workers reached their private terminal products. B의 남은 창은 유휴였으며 A는 자신의 창에서 실행했다.

## Controls and limitations / 대조군과 범위

Two ordinary cooperative prime workers exited 12/12. All six invalid CLI tables were rejected (controls.log). The initially consulted Windows local clone lacked this integrated object. Exact public branch fetch succeeded. The first host score script had a Python comprehension syntax error (replay-first.log). Its correction replayed the original raw data. There were no guest retries. An unused cooperative package containing the counter-reading ELFs was built; the actual compatibility control used two established prime ELFs.

Open source issue: the syscall path passes the original exception-entry counter through service processing to select. A syscall spanning an edge may therefore select the previous slot. The five boots did not force such an edge crossing. Full contract disposition is held for correction and regression evidence. 시스템 호출 처리 중 경계를 지나는 경우의 선택은 소스상 미해결이다. 기존 #219 계열의 1 ms 초과 기록은 유지하며 이 결과는 QEMU의 해당 첫 부팅에 한정한다.

Files: score.json holds per-edge absolute values. runs/*.tar.gz contain UART, complete captured process spaces, launch argv, registers and external exception traces when enabled. Per-package manifests bind source and ELFs. The full original Git archive is retained separately on D:.
