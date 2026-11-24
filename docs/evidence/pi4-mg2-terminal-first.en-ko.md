# Pi 4 MG2 Stage 1 independent assessment

Published collector 781d581f33431d1740e9f0ba8935264f3f88771d; stage plan 32ae8fb1893d874286fb4296ce4ce17373849b07. Cohort edfc775f1edd3c80c8d69ab754c0b5b27064bb8cd41c77ab547abd14db675ee7. Guest source, three linked ELF/image digests, original archive and QEMU digest are in cohort metadata. All six first positions ran singly under the stored MTTCG arguments. No replacements. Every position was copied and rehashed on D: before advancement. A harness newline error after freeze preceded all launches. The original script and resumed launch loop are retained.

Positions 1, 2, 3, 4 and 6 reached 0x1d. Each CPU0 prefix covers [0,16), CPU1 [0,9), both unsealed, with zero lost rows. M lease 1 reached indices 16, 32, 65536. Its final SVC frame stored X0–X4 = 0, 2147516416, 4096, 18080812676020141577, 65536. Independent recomputation and linked SVC PCs agree. CPU1 completed P1 lease 2 at 16/136 with count 1 and digest 152, retired it and posted 0x18001 toward CPU0. No CPU0 IRQ entry or resumed M appears in these traces.

Position 5 (clustered first) reached MG END. Its two sealed committed prefixes [0,37) and [0,19) match every UART row, with zero lost. Four summaries cover opening P1, subsequent P1, P0 and M. M's final checkpoint was already committed on lease 1 before CPU0 IRQ entry. Saved frame 593312, SP 594144, X21=0, ELR 0x82038. Lease 5 restored the same frame on CPU0 and the external ERET returned to 0x82038, a load after the checkpoint loop. No owner-bound M checkpoint follows. The frozen replay exits 1 with “M checkpoint progression/cookie” (see recorded return code for command exit). Completing arithmetic and resuming a frame did not meet the strict loop-PC/work condition. This diagnostic does not change the original 0/6 result.

QMP register dumps and QEMU thread IDs are retained per position. The first QMP command followed the terminal quiet interval. Total comparative timing overhead was not measured. No Stage 1 record orders the modeled pending edge against the last M add on the five failures. Physical Pi behavior remains untested.

## 한국어

여섯 최초 실행 중 1·2·3·4·6번에서는 CPU0의 M이 IRQ 진입 전에 최종 합 2147516416을 들고 SVC에 진입했다. CPU1은 P1 리스 2를 반납하고 CPU0으로 SGI를 보냈다. 두 코어의 미봉인 로그에는 유실이 없다. SGI의 GIC pending 전환과 M의 마지막 덧셈 사이의 순서는 확정할 수 없다.

5번에서는 양쪽 로그가 봉인됐고 RAM의 56개 행이 UART와 모두 일치했다. CPU0이 IRQ로 저장하고 리스 5로 복원한 실제 주소 0x82038은 산술 루프 이후였다. 복원 뒤 M의 체크포인트가 없어 공식 재생 검사가 실패했다. 기존 자격 판정은 0/6이며 실물 Pi 4 검증은 수행하지 않았다.

[Six raw cases and replay / 최초 6건 원본 및 재생](pi4-mg2-terminal-first.zip) · SHA-256 71b1edb10b5178a7a8924d2fc2d01db99e1c260f07a6c7c67e70f8e7e6858c25.
