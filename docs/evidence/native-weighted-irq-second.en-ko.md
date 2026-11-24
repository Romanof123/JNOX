# CHG-088 · Independent verification of the IRQ and startup corrections / IRQ 및 기동 수정 독립 검증

Three of four weighted first runs met the frozen CHG-088 limits at `af952e34cea8fbe0aef5c7b4cb1f945a2884fff5`. All four completed both native processes and passed the first-IRQ checks. The separate startup compatibility probe passed. Default-clock share missed two lease limits and one prefix limit. [Workload #177](https://github.com/Romanof123/JNOX/issues/177) · [Correction #183](https://github.com/Romanof123/JNOX/pull/183) · [Open share timing #184](https://github.com/Romanof123/JNOX/issues/184)

수정본에서 가중 작업 네 건 모두 첫 IRQ를 처리하고 두 프로세스를 종료했다. 결정적 시계의 지분·깨움과 기본 시계의 깨움이 전체 기준을 충족했다. 기본 시계의 지분은 두 실행 구간과 한 누적 편차에서 기준을 넘었다. 단일 프로세스 기동 호환성은 통과했다. [동결 기준](https://github.com/Romanof123/JNOX/blob/7549e57614221f4e6f71db32b0b369ef390e97f7/docs/scheduling-release-cases.md)

| Revised first case / 수정본 첫 실행 | UART | EL0 returns / IRQ / SVC | Full replay / 전체 판정 |
|---|---|---:|---|
| Share, default | END | 38 / 36 / 2 | FAIL · leases 7, 10 and prefix 7 |
| Wake, default | END | 26 / 15 / 11 | PASS · 3 wakes |
| Share, icount3 | END | 38 / 36 / 2 | PASS · 36 closures |
| Wake, icount3 | END | 26 / 15 / 11 | PASS · 3 wakes |
| Startup, default | EXIT 37 | 2 / 0 / 2 | PASS · write returned −38, errno 38, zero stdout bytes |

At 62,500,000 ticks/s, Q = 625,000 ticks, H = 781,250, G = 156,250. Default share leases 7 and 10 took 807,801 and 782,806 ticks, with gaps of 2,608 and 2,090. At closure 7, 2A−B reached 1,612,999 against a ceiling of 1,562,500. The 36-closure totals were A = 8,893,158 and B = 17,722,813. The terminal share bound passed. The deterministic totals were A = 7,500,024 and B = 15,000,048. Both totals omit the two finite exit leases. [Frozen limits](https://github.com/Romanof123/JNOX/blob/7549e57614221f4e6f71db32b0b369ef390e97f7/docs/scheduling-release-cases.en.md)

기본 시계에서 7·10번째 실행 구간은 H를 각각 26,551틱과 1,556틱 초과했다. 7번째 누적 편차는 상한을 50,499틱 초과했다. 36회 종료 시점의 지분은 두 시계 모두 최종 허용 범위에 들었다. 아래 표와 JSON의 모든 시각은 게스트 카운터 틱이다. 각 깨움 동안 B의 서비스 증가는 0이고, A가 완전한 구간 세 번을 수행했다.

| Clock / 시계 | Round / 회차 | I−D | W−I | R−W | S(max)−R | S(max)−D | A solo / 단독 | A full / 완전 | Intervening A / 개입 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| default | 1 | 125514 | 11826 | 751146 | 139630 | 1028116 | 2460935 | 3 | 1 |
| default | 2 | 110928 | 910 | 736476 | 105696 | 954010 | 2478161 | 3 | 1 |
| default | 3 | 116157 | 734 | 147485 | 128215 | 392591 | 2468746 | 3 | 1 |
| icount3 | 1 | 2 | 1031 | 626308 | 970 | 628311 | 2495012 | 3 | 1 |
| icount3 | 2 | 2 | 1031 | 626308 | 970 | 628311 | 2495013 | 3 | 1 |
| icount3 | 3 | 2 | 1031 | 2271 | 963 | 4267 | 2495013 | 3 | 1 |

D is the requested deadline, I the IRQ entry, W the accepted wake, R the first resumed B return to EL0, and S the clock sample bounded by its producing syscall. All six sample intervals narrowed to one tick. The five interval limits in column order are 156,250, 156,250, 1,093,750, 156,250 and 1,562,500 ticks. Each weighted replay verified the two ASIDs, distinct writable data and stacks, mapped ELF, private work and native exits 41 and 42.

D는 요청 기한, I는 IRQ 진입, W는 깨움 수락, R은 B의 최초 EL0 재개 직전, S는 재개한 프로그램이 받은 시계 표본이다. 여섯 표본 모두 해당 시스템 호출 구간과 교차한 뒤 한 틱으로 확정됐다. 네 실행 모두 두 주소 공간, 분리된 데이터·스택, 계산 결과와 종료 41·42를 확인했다.

Each weighted trace reached EL0 at 0x400000, entered the timer IRQ, then returned to EL0. The journal recorded full IAR 30 and control 5 on every acknowledged IRQ. Both linked handlers load the IAR, read the timer operands, disable the timer, check ID/control/time and store the same full IAR to EOI. The non-IRQ branch skips the IAR load. The share load/disable/EOI PCs are 0x40201d1c / 0x40201d28 / 0x40201d54. Wake uses 0x40201d24 / 0x40201d30 / 0x40201d5c. The rebuilt old startup selector entered EL0 at 0x400000, returned −38 on the unsupported write, and exited 37. [IRQ defect #181](https://github.com/Romanof123/JNOX/issues/181) · [Startup build #182](https://github.com/Romanof123/JNOX/issues/182)

수정본의 모든 타이머 IRQ 기록은 전체 IAR 30, 제어값 5였다. 두 커널의 링크 결과에서 IAR 읽기, 타이머 비교값·제어값 읽기, 타이머 비활성화, 조건 확인, 같은 IAR 값으로 EOI 쓰기 순서를 확인했다. 기존 기동 프로브는 0x400000에서 EL0에 진입했고 미지원 쓰기에서 −38을 받은 뒤 37로 종료했다.

The predecessor at `56d6e0b7cb7183fdc1b9b06275050ce64254e6f6` emitted FAIL in its four weighted first boots. Three first RAM captures identified guard 11. The predecessor's share/icount3 RAM capture was incomplete and the startup build reported a duplicate `.Lpark`. Its original compiler output and its original verdict are preserved in the archive's previous-cohort directory. The revised captures used clean Linux Rust 1.95.0 release builds, clang/LLD 14, QEMU 6.2.0 Cortex-A72 virt/GICv2, 128 MiB, one vCPU and the same weighted application SHA-256 across both cohorts. The revised verifier froze share/default, wake/default, share/icount3, wake/icount3, then startup/default. The predecessor had placed share/icount3 second. Every arm/clock was run once on the revised images. No sample was replaced. [Original candidate #178](https://github.com/Romanof123/JNOX/pull/178) · [Correction contract](https://github.com/Romanof123/JNOX/blob/af952e34cea8fbe0aef5c7b4cb1f945a2884fff5/docs/native-weighted-irq-order.en-ko.md)

원본 네 건은 첫 IRQ에서 실패했고 원본 RAM 세 건에서 가드 11을 확인했다. 원본 지분/icount3은 첫 RAM 수집이 미완료였고 원본 기동 빌드에서는 중복 레이블 오류가 발생했다. 수정본은 같은 응용 ELF를 사용해 다섯 건을 각각 한 번 실행했다. 수정본은 기본 시계 두 사례를 먼저 실행했으며 원본은 지분의 두 시계를 먼저 실행했다. 실험 범위는 단일 vCPU QEMU이며 물리 장치나 제품 시간 보장은 별도 검증 대상이다.

[Raw captures, compiler logs and replay outputs / 원자료](native-weighted-irq-second.zip) · [Verdicts, intervals and digests / 판정·구간·해시](native-weighted-irq-second.json)

Extract the ZIP and verify `sha256sum -c SHA256SUMS` from its root. Run `python3 independent.py` with Python 3.10+ and pyelftools 0.32. Use the full candidate checkout with the retained case directory and the corresponding `replay/*.capture-digest` for the repository replay. The expected replay exits are 1, 0, 0, 0 and 0 in the table order.

압축을 풀고 루트 디렉터리에서 체크섬 목록과 독립 판독기를 실행하면 원자료를 검증할 수 있다. 수정본 저장소의 판독기에 각 사례 디렉터리와 해당 세션 해시를 입력하면 표의 순서대로 종료 코드 1, 0, 0, 0, 0을 재현한다.
