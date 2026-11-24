# CHG-088 · Two native weighted processes, first independent results / 네이티브 가중 프로세스 최초 독립 검증

**FAIL. Four of four first boots reached the first A timer IRQ and printed `NATIVE VRUNTIME FAIL`.** None resumed B or completed the planned share or wake horizon. Development and Integration can track the IRQ failure and startup regression independently. [Work item #177](https://github.com/Romanof123/JNOX/issues/177) · [Candidate #178](https://github.com/Romanof123/JNOX/pull/178) · [First IRQ #181](https://github.com/Romanof123/JNOX/issues/181) · [Startup build #182](https://github.com/Romanof123/JNOX/issues/182)

**최초 네 번 모두 실패했다.** A의 첫 타이머 IRQ까지 진입했으며 B의 실행과 종료, 36회 점유율 측정 및 절대시각 깨움 세 차례는 관측되지 않았다. IRQ 실패는 #181, 기존 기동 프로브의 빌드 회귀는 #182에서 추적한다.

## First observations / 최초 관측

Independent clean checkout `56d6e0b7cb7183fdc1b9b06275050ce64254e6f6`. The English and Korean CHG-088 documents match the frozen parent `7549e57614221f4e6f71db32b0b369ef390e97f7`. Two separate one-job release builds used Rust 1.95.0, Clang/LLD 14 and the same native C executable. QEMU 6.2.0 ran AArch64 virt, Cortex-A72, GICv2, 128 MiB, one vCPU and TCG. The separately declared deterministic mode used `shift=3,align=off,sleep=off`. The original run order and input hashes are in the archive. [Frozen plan](https://github.com/Romanof123/JNOX/blob/7549e57614221f4e6f71db32b0b369ef390e97f7/docs/scheduling-release-cases.en.md) · [동결 기준](https://github.com/Romanof123/JNOX/blob/7549e57614221f4e6f71db32b0b369ef390e97f7/docs/scheduling-release-cases.md)

| First case / 최초 사례 | Guest UART | External ERET / IRQ / SVC | Interrupted EL0 PC | First RAM / journal | Checked A chunks |
|---|---|---:|---|---|---:|
| Share, default | FAIL | 1 / 1 / 0 | 0x40056c | failure 11 | 10,142 |
| Share, icount3 | FAIL | 1 / 1 / 0 | 0x40056c | no first RAM, ENODATA | — |
| Wake, default | FAIL | 1 / 1 / 0 | 0x40056c | failure 11 | 13,184 |
| Wake, icount3 | FAIL | 1 / 1 / 0 | 0x400590 | failure 11 | 482 |

All four UART streams start with BOOT and READY. Each independent QEMU trace shows the ERET to EL0 PC 0x400000 and the IRQ from A into EL1 vector 0x40200c80. No second ERET followed. The first RAM files each contain two mapping records and A's first selection/frame, with zero committed STOPs, no wake acceptance and no exit. On share-icount3 the original capture reported `[Errno 61] No data available` and saved UART and exception history. The supplied replay returned 2 for its incomplete capture and 1 for each of the other guest failures.

The three complete RAMs had a 62,500,000 Hz counter and Q=625,000 ticks. At failure their page tables showed ASIDs 1 and 2, disjoint RW and 16-page stack backings, equivalent user VAs, a shared read-only horizon page still zero, expected RX/RO/RW ELF permissions and the allocated SDK ABI note. The result symbol was at 0x402008 in each process with physical pages 0x40233000 and 0x40288000. Direct recomputation of the C arithmetic matched A's stored values. Each B result remained all zero. The linked user ELF matched its embedded copy in physical RAM. Both SDK exits, cross-owner frame restoration, all 36 charged share prefixes, post-horizon tail service and every wake D/I/W/R/S interval remain unobserved. No interval can be compared with its frozen limits. The independent decoder retains each mapped page, both product hashes, frame details and all available first rows.

최초 RAM 세 건에서 62.5 MHz, Q=625,000틱, ASID 1·2, 독립 RW/스택 페이지와 각 결과의 체크섬을 확인했다. A의 계산값은 세 건 모두 독립 계산과 일치했고 B의 결과는 모두 0이었다. share-icount3는 UART·예외 기록에 실패가 남았으나 호스트 캡처 오류로 최초 RAM이 없다. 36개 점유율 접두 구간과 세 차례 깨움의 D/I/W/R/S 구간, B 복귀·두 앱 종료는 판정할 관측값이 없다.

## First IRQ and retained controls / 첫 IRQ와 기존 대조군

The three first RAMs independently localize failure 11 to the interrupt ID, timer status or compare guard. One separately labelled icount3 diagnostic reused the exact share image. Its stopped registers, RAM and disassembly show GICC_IAR INTID 1023, timer control 5, compare 841078 and IRQ-entry tick 841080. The linked handler disabled the timer before reading GICC_IAR and branched on INTID differing from 30. The diagnostic is stored separately from the four first observations. [IRQ defect #181](https://github.com/Romanof123/JNOX/issues/181)

The startup-probe built at the parent and completed its ordinary default-clock guest, SDK exit 37 and raw replay. The candidate assembled the startup user ELF, then its Rust kernel build exited 101 with `symbol '.Lpark' is already defined`. Its startup guest was never launched. On the pinned deterministic clock, all five legacy EL1 selectors had identical parent/candidate PT_LOAD contents, segment mappings, UART and exception traces. The complete ELF file hashes differed. The four ordinary share/wake plain/trace selectors on each source passed their supplied replays. Both fixed `vruntime-bench` guests emitted `VR PASS`. These observations cover only the named QEMU selectors. [Startup defect #182](https://github.com/Romanof123/JNOX/issues/182)

별도 동일 이미지 진단에서 IRQ ID는 기대한 30 대신 1023, 타이머 제어 값은 5, 비교값은 841078, 진입은 841080틱이었다. 원래 세 RAM은 실패코드 11까지 확인하며, 구체적인 ID는 별도 진단에 근거한다. 부모 기동 프로브는 종료 37·리플레이 통과, 후보는 `.Lpark` 중복으로 커널 빌드 실패·게스트 미실행이다. 기존 EL1 다섯 선택기는 부모/후보의 적재 세그먼트와 UART·예외 기록이 같았고 가중 작업 네 종류의 리플레이 및 고정 벤치마크 종료 표식을 확인했다.

## Retained evidence / 보존 자료

[Compact raw archive](native-weighted-first.zip) · [Machine-readable verdict and hashes](native-weighted-first.json)

The archive contains original kernels and user ELFs, first serial/exception streams, three first RAMs, capture and replay statuses, build logs and manifests, parent comparisons, the separate diagnostic and a SHA-256 file manifest. With Python 3.10+ and pyelftools 0.32, extract the archive and run `sha256sum -c SHA256SUMS` and `python3 independent.py` at its root to reproduce the partial first observations directly. The exact two semantic defects remain open at the candidate revision. Product timing, unrestricted process services and physical-board behavior are outside this engineering observation.
