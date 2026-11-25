# Independent verification of native RM/DM periodic applications / 네이티브 RM·DM 주기 앱 독립 검증

Candidate 4a0c17a7025d663d90b67535ccc27b594277ec08, accepted main 33ee8c35a62ede54987f9af4ae51d3f2fcea3d35, frozen design 0d2341950ca908eff69c15f2f2574212646b8ce6. WSL, Rust 1.95.0, QEMU virt/GICv2, one Cortex-A72, TCG, ordinary 62,500,000 Hz physical counter. First plain and QEMU exception-traced boots ran serially for every position, with QMP observations in both. All eighteen original positions and four supplemental positions reached their expected exit markers. Every periodic terminal capture reported zero dropped rows. Raw UART, command, progression, RAM, ELF, symbol and build evidence accompanies this report.

원래의 9가지 구성은 일반 실행과 외부 추적 실행을 한 번씩 수행했다. 별도 고정 CRC 실험도 정책별 두 방식으로 수행했다. 최초 실행을 모두 보존했다. 모든 주기 실행에서 이벤트 손실은 0이었다.

## Measured jobs / 실측 결과

Misses and cancellations identify slot and generation. B1 delta is its physical completion counter minus its nominal absolute deadline, in ticks. The A/B service column gives measured CPU service accumulated through exit, excluding dispatch. The input phase jitter column gives the largest observed difference between release and its sampling boundary in that capture, excluding cancelled future work. These samples supply no platform-wide upper bounds.

표의 A0는 슬롯 A의 0세대다. B1 차이는 물리 완료 시각에서 B1의 마감을 뺀 틱 수다. 서비스는 디스패치 시간을 제외한 실제 누적 실행 틱이다. 공개 관측 지연은 이 실행에서 관측한 최댓값이다.

| Position | Initial slot | Misses | Cancelled | B1 delta | Service ticks by slot | Max observed release lag |
|---|---:|---|---|---:|---|---:|
| dmover-plain | 1 | A1 | — | -2488613 | 6888270,23743 | 19360 |
| dmover-traced | 1 | A1 | — | -2315261 | 7288813,388122 | 135765 |
| dmref-plain | 1 | — | — | -2483813 | 39158,26801 | 7771 |
| dmref-traced | 1 | — | — | -2240608 | 424787,380527 | 127505 |
| equal-plain | 0 | — | — | -5597173 | 23213,38938 | 7840 |
| equal-traced | 0 | — | — | -5211588 | 778281,416676 | 137915 |
| rmover-plain | 0 | A1,B1 | — | 3166641 | 6890200,20727 | 7792 |
| rmover-traced | 0 | A1,B1 | — | 3832988 | 7435211,378729 | 131723 |
| rmref-plain | 0 | — | — | -2486110 | 45598,23137 | 7385 |
| rmref-traced | 0 | — | — | -2241521 | 396921,381126 | 123603 |
| sleep-plain | 0 | A0,A1,A2 | A1,A2,A3 | — | 36892 | 8451 |
| sleep-traced | 0 | A0,A1,A2,A3 | A1,A2,A3 | — | 514305 | 116526 |
| solo-plain | 0 | — | — | — | 29012 | 7894 |
| solo-traced | 0 | — | — | — | 722382 | 120025 |
| three-plain | 1 | — | — | -2487299 | 43463,24935,16794 | 10031 |
| three-traced | 1 | — | — | -2261735 | 407569,392828,703911 | 136257 |
| fixeddm-plain | 1 | A1 | — | -2466751 | 11528304,27209 | 15660 |
| fixeddm-traced | 1 | A1,A2 | A3 | -2231856 | 12265048,399751 | 141578 |
| fixedrm-plain | 0 | A1,B1,B2 | — | 7744212 | 11457390,18820 | 10505 |
| fixedrm-traced | 0 | A1,A2,B1,B2 | A3 | 8914655 | 12476859,387158 | 128699 |

## Private outputs and continuation / 사설 데이터와 실행 재개

Reference C byte counts were 16,384 / 16,640 / 16,896 with CRC e81722f0 / b03637e8 / 82e8647b. Rust processed 8,192 / 8,448 / 8,704 bytes, producing all sixteen bins equal to 512 / 528 / 544 respectively. Both images wrote sequence [1,2,3], waited twice and exited 17/29. The shared ELFs gave first dispatch A→B under RM and B→A under DM. Independent replay checked slot-private data at distinct physical aliases, cumulative service, sequence, anchored releases/deadlines, reported misses, cancellations, ready and suspended masks, comparator, dispatch ranks, return PC/SP/frame and status. Raw physical comparison for every committed generation is in replay.json and fixed/replay.json.

기준 앱은 두 정책에서 같은 ELF를 사용했다. 최초 실행 순서는 RM A→B, DM B→A였다. 두 번 대기한 뒤 C는 17, Rust는 29로 종료했다. 모든 세대의 바이트 수, CRC, 16개 빈과 보고 순서를 사설 메모리에서 독립 계산값과 대조했다.

| Clock-polled overrun first | A1 blocks | A1 CRC | Measured ns |
|---|---:|---|---:|
| dmover-plain | 37576 | 23767b0a | 110000224 |
| dmover-traced | 121 | c8a1e301 | 110261104 |
| rmover-plain | 37568 | bb11a0d6 | 110001152 |
| rmover-traced | 123 | 55b8ccf0 | 110799664 |

Both original DM runs selected B1 when A sampled its clock (cause 5), then restored A at 0x4001a0, SP 0x1010f40. Both fixed-work DM runs instead recorded an EL0 physical-timer IRQ twice during A1, at 0x4001ec inside crc_ramp, SP 0x1010f50, and restored A at that address and stack pointer after B1 and B2. The external exception record confirms IRQ entry and exception return. For the 64 / 131072 / 66 ramps, stored completed-block counts, sequences, byte counts and recomputed CRCs e81722f0 / fa914cb6 / 82e8647b all matched. RM let A continue while B1 waited. Per-generation deltas and additional misses under trace are retained above. Frame identity, pointer and products were checked. No byte-for-byte stack snapshot was taken at the instant of suspension and resumption.

원래 과부하 DM 실행은 시계 읽기 시스템 호출에서 B1으로 전환했다. 보충 실험에서는 고정 32 MiB 연산 중 실제 타이머 IRQ로 B1과 B2가 각각 선점했고, 같은 PC·SP로 돌아와 CRC를 완성했다. 스택 내용 전체의 선점 직전·직후 비교는 수행하지 않았다.

## Edges, analysis and compatibility / 경계·분석·호환성

One delayed slot yielded and returned to its persistent local on three activations (16/32/48, exit 43). The equal-key yield transferred ownership to the waiting peer. Three independently linked processes all returned their own products. Sleep/backlog exited 53 after its requested absolute sleep, cancelled exactly three pending activations, and produced the misses shown above. The default cooperative profile returned -1 with ENOTSUP=95 and exited 59. The original eleven commands omitted the primary ELF, and seven reached the generic missing-program diagnostic. Four returned argument parsing errors first. A separate targeted pass supplied the verified primary ELF in all seventeen CLI cases and matched the individual numeric, cardinality, duplicate, orphan and mixed-field diagnostics. Ten raw metadata inputs independently reached the expected diagnostic in the already-built Cargo build-script executable. Exact inputs and stderr are in supplement-selector/records.json. The 152 serial Rust tests passed including exact D / D+1, duplicate observation, chronological-versus-batched sampling, overflow and frequency-dependent conversion rejection. Exact D / D+1 evidence is deterministic host evidence. No frequency-rejection guest was launched.

단일 지연 공개·동률 양보·세 프로세스·다음 공개를 넘긴 수면·조기 종료·프로필 밖 호출을 확인했다. 원래 명령 11개 모두 주 ELF가 없었으며 7개는 프로그램 누락, 4개는 인수 파싱 오류로 거부됐다. 별도 검증에서 실제 ELF를 지정한 명령 17개와 Cargo 빌드 스크립트에 직접 전달한 메타데이터 10개가 의도한 사유로 거부됐다. 호스트 테스트 152개가 통과했다. 마감과 정확히 같은 틱 및 다음 틱은 결정적 호스트 테스트로 확인했다.

For declared q=10 ms, both WCET=1q, zero modeled blocking, jitter, interrupt and dispatch costs, host RTA returned A/B responses (1q,2q) RM and (2q,1q) DM. Physical WCET, IRQ, blocking, dispatch and release-jitter upper bounds are unavailable. Event-level observations are recorded in raw runs. They cannot establish these upper bounds. The archived numeric RTA input was emitted after guest observation using the values fixed in the prelaunch protocol. Each actual guest epoch was drawn by the kernel at startup and recorded afterward; the prelaunch protocol fixed the rule rather than a counter value. Per-position argv/address manifests were emitted immediately before each launch.

선언한 비용으로 분석한 응답은 RM (1q,2q), DM (2q,1q)이다. 실제 플랫폼의 최악 비용과 지터 상한은 측정하지 않았다. S의 선택 규칙을 고정했고 실제 값은 부팅 때 기록했다.

Accepted main and candidate each returned 37 for ordinary startup and 43 for an accepted EDF release. Both linked RM and DM loss controls reached CONTROL END on both revisions; all four ordinary-clock control replays reported the existing actual-delivery failure. Historical #94 v3 is separately recorded as 14/23 firsts and 15/23 with its retry, eight unqualified default-clock cases, and later #94/#213 single-decision 0/8 healthy. These counts come from the frozen design. This run did not repeat those historical cohorts. There was no physical board or product test.

기준 main과 후보 모두 일반 시작 37, EDF 43으로 종료했다. 양쪽의 기존 연결형 RM/DM 제어는 CONTROL END에 도달했으나 실제 전달 조건은 네 번 모두 미충족이었다. 과거 #94 최초 14/23, 재시도 포함 15/23, 기본 시계 8건 미충족, 이후 #94/#213 건강 조건 0/8은 별도 기록이다. 실물 보드와 제품에서는 실행하지 않았다.
