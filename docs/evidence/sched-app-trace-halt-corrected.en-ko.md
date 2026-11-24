# Independent CPU0 trace acquisition / CPU0 독립 수집

0174fe3b67a26f75e5c44a02a992d28297707219. Eight locked, single-job builds on WSL ext4 and eight separate AArch64 virt cold boots passed boot, seal and decode on their first attempts. All showed running with one integer-indexed CPU0, then paused before extraction. The plan IDs identify acquisition directories. The public wrapper generated separate run UUIDs listed in each original manifest.

8개 최초 실행 모두 실제 CPU0 실행·정지 응답과 원본을 봉인했다. 원래 33d8fbf의 8개는 모두 halted.json 누락으로 거부됐으며 원본을 그대로 보존했다. 수정 전 35개 중 해당 8개만 새 결과로 교체 가능하고 나머지 27개는 이전 근거를 재사용한다.

| Mode + full-frame flag | E−W | Q−W | S−Q | E−S (trace) | IRQ overlap with E−S | kernel/app drops |
|---|---:|---:|---:|---:|---:|---|
| active0 | 2482307 | 4435 | 2471805 | 6067 | 4540 | [0, 0] |
| active1 | 2486840 | 4600 | 2476004 | 6236 | 4464 | [0, 0] |
| disabled0 | 2482199 | — | — | — | — | 0/0 |
| disabled1 | 2489591 | — | — | — | — | 0/0 |
| off0 | 2463266 | — | — | — | — | — |
| off1 | 2457661 | — | — | — | — | — |
| saturate0 | 2484982 | — | — | — | — | [22, 1] |
| saturate1 | 2488021 | — | — | — | — | [22, 1] |

Ticks at 62.5 MHz, bound 14,375,000 (23U). oracle.json gives each original packet output, counts, timer duration, stop/start gaps, lateness, frame and wall decomposition from the raw witness and ELF. frames.json independently checks the full-frame restores against ERET PCs. The independent audit reuses the earlier independent C packet and witness algorithms after sealing fresh positions. Disabled recorded zero. Saturation preserves only its first four records and prevents latency attribution. IRQ service can overlap dispatch-to-receipt. No WCET, causal overhead estimate, measured mask duration, multicore or hardware claim.

2026-11-25 KST: the recorder implementation is dated 07:13:10. The prospective halt contract landed at 07:17:55 and the host-only correction at 07:20:07. Relative to 33d8fbf the kernel, build selectors and older client source are identical. The changed common helper defaults record_halt to false. The unchanged mlfq_wake_capture.py produced UART, register, IRQ, RAM and legacy qmp.json on one extra corrected-source boot. Original positions 9–35 are historical controls, not 27 fresh boots. Original rejections are verified against the original sealed artifact hashes.

Producer and reader order: the writer masks IRQ, writes body, release-publishes sequence, then advances count. The trace wrapper obtains status/CPU replies, stops, obtains and records paused, then requests registers and RAM. Three protocol tests (including six negative transitions), five decoder tests and three producer tests passed. Command order and zero reads on those failures were observed with the mock transport; live QMP files contain actual response bodies, not a command transcript. IRQ mask/pending behavior comes from a model and code inspection.

Inspection limitation: the producer-side QMP checker and offline decoder compare cpu-index to 0 using Python equality without an integer-type check. Malformed boolean false and floating zero compare equal. The stop command checks for a return envelope without checking its empty-object payload. Real QEMU returned integer index 0 in all eight positions. The focused tests do not cover these malformed values.

검증 범위는 WSL/QEMU 단일 CPU와 연결된 워크로드다. 모의 프로토콜 시험에서 잘못된 상태·오류 후 읽기가 차단됐고, 실제 게스트에서는 정상 상태 전이를 확인했다. 비정상 CPU 인덱스의 엄격한 정수형 판별과 stop 반환 형식 검사는 코드상의 제한이다. 마스크 시간·최악 실행시간·실기기 오버헤드는 측정하지 않았다.

[First corrected guest receipts and original files / 정지 상태 수정본의 최초 실행 자료](sched-app-trace-halt-corrected.zip) · SHA-256 4223962d49283f4d78179fc4caa1c8b2350691e1022dc86d5e32dcae6ec61c93.
