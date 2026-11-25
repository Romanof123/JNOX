# Supplementary share/icount3 observation / share·icount3 보충 관측

**Comparator PASS. Candidate PASS. / 비교군·후보 모두 통과.** Public addendum be655161737953d4c34999e08ae1ed142fdbecef authorized two new cold boots, in that order. Both used WSL ext4 /dev/sdd for all live files. QEMU exited after capture; each source archive and its raw SHA-256 manifest were sealed before transfer. Both D: copies matched the independent source digests and both scores matched after destination-side extraction. The original candidate 04 remains permanently partial. Original evidence c86078a8 includes the fresh comparator default-share timing failure and the three observed candidate passes. This supplement completes observational coverage of all four finite virtual candidate configurations. Release owns the acceptance decision.

공개된 보충 계획에 따라 비교군 다음 후보 순서로 각각 한 번씩 콜드 부팅했다. 실행 중 생성되는 파일을 모두 WSL ext4에 보관했다. 원본 봉인 후 D: 사본을 해시 대조하고 압축을 풀어 다시 판독한 결과가 일치했다. 최초 04의 RAM 결측과 최초 기본 클록 비교군의 시간 한계 실패는 그대로 유지한다. 보충 자료를 합치면 후보 네 가상 구성의 관측 자료가 모두 확보되며, 최종 수용은 Release가 판정한다.

| share/icount3 | Comparator / 비교군 | Candidate / 후보 |
|---|---:|---:|
| IRQ / SVC / EL0 ERET | 36 / 2 / 38 | 36 / 2 / 38 |
| A, B service through closure 36 / 36회까지 서비스 | 7500024, 15000048 | 7500012, 15000024 |
| Denominator / 분모 | 22500072 | 22500036 |
| A fraction / A 비율 | 1/3 | 1/3 |
| Prefix discrepancy range, all 36 / 접두 36개 범위 | 0…1250004 | 0…1250002 |
| Maximum lease / 최대 임대 | 625002 | 625001 |
| Maximum preceding gap / 최대 간격 | 1174 | 1096 |
| A, B voluntary tail service / 자발적 종료 꼬리 | 401, 741 | 466, 866 |

Both retained the exact user ELF 9d3c6350a04536da091dd7bc77cfdcc81729f8baa431d6315e5258c28da67206. Comparator kernel eb1465b85787b8eeee9a54712f2d6130de0db7352bb9dc984a60b034e9a15c1c; candidate kernel 2cac6b96389e5db3b646f03f323ff0ecc7c1e65564a65558fbec9fdc6d36b322. ELF entries were matched to original images before launch. QEMU 0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8 retained the original command except for output paths. The raw archives include the exact commands, affinity, physical RAM extent and both independent scores. Q=625000, H=781250, G=156250, F=62500000. Every eligible ready-set prefix and final share tolerance passed. Every grant including voluntary exits had positive physical charge and met H, and every noninitial preceding gap met G. Full IRQs reached their armed compare. Notification was first recorded at closure 36 and remained latched through the tails.

원래 실행 파일·진입점·QEMU·클록·작업 인자와 수치 한계를 사용했다. 모든 임대의 실제 서비스, 초기 임대를 제외한 모든 선행 간격, 36개 접두의 분배 오차와 최종 비율이 통과했다. IRQ는 실제 비교점 이후에 발생했다. 후보는 36회에서 통지를 게시하고 이후 두 종료까지 유지했다.

Candidate raw RAM agreed with every UART journal row. The replay joined each IAR30 and EOI to the recorded compare, architectural IRQ entry, saved PC/SP and external ERET PC, and checked the actual ERET opcode at each linked return site. Candidate frames 0x4020f000 and 0x4020f340 used ASIDs 1 and 2. Both initially entered at PC 0x400000 and SP 0x1010fb0. Both final resumptions used PC 0x400570, SP 0x1010f30 with linked ERET at 0x40201518; the first return site was 0x40201200. The final two SVCs used ELR 0x400020, SP 0x1010fb0 and exited from their own frames. Independent virtual-runtime fractions, lower-slot tie choices, monotone floor and owner service totals matched all rows. Both executable images had the expected role/arm and private writable data and stacks. The completion word was shared read-only, its remainder zero. Separate original comparator replay validated frame restoration, private images and terminal checksums.

후보의 RAM과 UART 행을 대조하고, 물리 타이머 IAR30·EOI와 외부 예외 및 ERET 기록을 연결했다. 두 프로세스의 프레임·ASID·복귀 PC/SP·종료 호출이 일치했다. 실제 서비스에서 독립 계산한 가상 실행 시간의 몫과 나머지, 동률 선택, 단조 증가 바닥값을 검증했다. 읽기 전용 통지 페이지와 각 프로세스의 사설 쓰기 페이지·스택을 확인했다.

| Final private result / 최종 사설 결과 | A (both images / 양쪽) | B (both images / 양쪽) |
|---|---:|---:|
| chunks / sequence | 5785 / 11570 | 11570 / 23140 |
| samples | 1480960 | 2961920 |
| sum / weighted sum | 7404800 / 72752160 | 14809600 / 112923200 |
| exit status / 종료 | 41 | 42 |

Destination-verified source archive digests / 원본과 대조한 사본 SHA-256

- Comparator raw archive 6c9c027015b7a0ed4b21feec4639650706fe13dff10b4bdfcbd4e5a624214835, session 30c8a76165c339d067d5e519a9f19a7c5f3b8e61a51be44876d56e8f8c312800.
- Candidate raw archive 2489abaf0777fc4e51cb565bca14e2f06fd56f828fc0c646ec63eeb68bc5071e, session 912ff4b33580caddb926cac0854b75b48c329341d35c1ba960bf039a1f227490.

Each stages.json includes source capture, score, archive, transfer and destination replay status with nanosecond wall timestamps, plus the raw capture stdout/stderr in each member. These observations apply to the named single-vCPU virtual machine and do not identify the source of the earlier ENODATA or characterize physical WCET.

각 단계의 타임스탬프 및 판정과 수집 로그를 포함했다. 단일 가상 CPU에서 관측한 결과이며, 이전 ENODATA의 원인과 물리 장비의 최악 실행 시간은 미확인이다.
