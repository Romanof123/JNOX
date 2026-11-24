# First-grant reservation QA / 첫 승인 예약 검증

**7/8.** Position 1 missed the 62,500-tick opening limit by 5,893 ticks. The seven other first boots passed their entire horizon. The eight-case ordinary gate stays open. / 최초 8건 중 7건 완주. 1번 실제 작업자 표지가 5,893틱 늦어 전체 통과 조건을 충족하지 못했다.

Candidate a4d2f340020f7e8666d04a113040207805bbfc9a, design 8badbe64ea5ab71e3ac542a9bd80a9ba29554def, technical base 3279b1ad48fe656ec9607d1fa438406a35d54d80. Isolated Windows Rust 1.95.0 locked size-release builds used one job. WSL native ext4 hosted eight distinct QEMU 6.2 TCG guests, Cortex-A72 virt/GICv2, one CPU, 128 MiB, default counter 62.5 MHz. All eight full UARTs ended before host timeout 124 at 15 s. The four paired selectors resolved to identical ELF hashes across modes. Guest files and zip members were rehashed on D: before launching the next guest. / 4개 앱의 일반·추적 쌍은 같은 ELF이며, 매 15초 최초 실행 후 다음 실행 전에 원본과 D: ZIP의 모든 파일 해시를 대조했다.

|#|App|Trace|#192 slot/status/reason (mask)|Candidate slot/status/reason (mask)|Marker − epoch, ticks|First charge, ticks|Captured grants|Prep slack, ticks|
|--:|---|---|---|---|--:|--:|--:|--:|
|1|aligned-wake|off|0/2/5 (3)|0/2/2 (31)|68393|13217|1|2982763|
|2|cbs-wake|on|0/2/2 (31)|34/0/0 (31)|60668|10086|17|3029696|
|3|aligned-saturated|on|0/2/5 (7)|10/0/0 (31)|57117|3017542|8|3027035|
|4|cbs-saturated|off|10/0/0 (31)|10/0/0 (31)|51599|3019475|8|3028642|
|5|aligned-wake|on|34/0/0 (31)|34/0/0 (31)|56684|10051|18|3003435|
|6|cbs-wake|off|0/2/2 (31)|34/0/0 (31)|54263|8763|17|3026393|
|7|aligned-saturated|off|10/0/0 (31)|10/0/0 (31)|61004|3011685|8|3027272|
|8|cbs-saturated|on|0/2/5 (15)|10/0/0 (31)|46571|3031740|8|3033612|

Position 1 returned to worker A at 3,233,558, marked real Rust work at 3,237,339 and trapped at 3,246,775 against epoch 3,168,946. Charge was 13,217 ticks to deadline 8. Its terminal operands are (3,237,339, 3,168,946). Other terminal operands are (0,0). Seven completions have 34 wake slots or 10 saturated slots. All 85 closed grants match real progress, saved PC/frame, indexed return, action and cumulative per-owner/deadline charge. Only traced positions 2, 3, 5 and 8 have external exception/ERET evidence; all have internal pre-ERET. The official decoder accepted eight streams. Direct and independent replay details, including observed failure, per-grant bounds, accumulators and allocation periods, are in direct.json and sealed.json. / 첫 부팅에서도 실제 작업과 복귀 및 13,217틱 과금을 확인했다. 실패는 실제 표지의 개시 한도 초과였다. 모든 85건의 복귀·작업·누적 부과를 실측 행과 대조했다.

The #192 first cohort recorded 3/8. Positions 2, 3, 6 and 8 newly completed, 4, 5 and 7 completed in both, and 1 advanced from mask 3 to an actual late marker. Current receipts all reach mask 31. Single cold observations per position do not isolate a cost or demonstrate repeatability. / #192 최초 3/8에서 이번 7/8로 바뀌었고 1번은 선택 전 중단에서 실제 작업자 표지까지 도달했다. 각 조건당 단일 관측이므로 특정 비용이나 재현율을 추정할 수 없다.

## Ownership, guards and linked layout / 소유권·시각·바이너리

Masked idle checks empty witness, pristine source, unused destination and initial return count. It builds a first record from the checked decision and private retirement state. A single raw write installs WITNESS. The shared reference survives close because storage is static and no later path rewrites the witness. Both preparation and IRQ read the live frame address, PC, mode, register, TPIDR, atomics and retirement/allocation inputs for both workers. IRQ confirms idle frame/return, unpublished source, matching destination owner/action, then release-publishes 3. Aligned-wake 0x402018e4–0x4020192c shows STLR, LDAR, sampled word passed to ledger_at, actual owner read from its bank, and word/owner checks. A missing destination and unrecognized word fail closed. Activation follows the post guard. Later grants own their records. Both IRQ and SVC enter close; the option is consumed before any accounting (0x40202dc0 clears its discriminant). / 마스크된 준비 경로가 정적 증거를 한 번 작성한다. 두 시점 모두 실제 프레임과 원자값을 읽으며, 게시 후 같은 권한값으로 뱅크를 조회해 소유자와 시각을 검사한다. 첫 승인은 불변 참조, 후속 승인은 자체 소유이며 두 종료 경로 모두 과금 전에 옵션을 소비한다.

Entry ≥ target = epoch, before ≥ entry, post ≥ before, armed ≥ post, planned ≥ armed. Before, post and planned must be ≤ checked epoch + 62,500. Selection independently checks both opening bounds. Aligned-wake disassembly has precommit guard, post lower/upper branches at 0x40201930–0x4020193c, and timer compare/control programming and physical readbacks at 0x402019b8–0x402019c8. Timer disable, EOI, barriers, actual transfer, pre-ERET and worker markers remain. Work and close retain measured elapsed and cumulative service, nominal end/cutoff and actual progress bounds. / 모든 시각 하한·상한, 타이머 비교·제어 레지스터 실측, 실제 복귀와 작업·기한별 과금을 유지한다.

Across four binaries the IRQ symbol shrank 2796→2564 bytes, idle grew 1020→1104, close grew 1184→1468, and WITNESS grew 264→320. The out-of-line validator occupies 336 bytes with two IRQ call sites and a preparation loop call. The bank resolver occupies 56 bytes. Aligned-wake close stack frame grew 240→256 bytes. Its common Rust work routine is 288 bytes in both, with distinct linked bytes; both 12-byte worker entry wrappers match. These are layout and stack costs; cycle cost was not isolated. All eight prepared before epoch (table). / 연결된 IRQ는 232바이트 줄고 종료 함수는 284바이트, 증거 저장소는 56바이트 늘었다. 준비 시한은 8건 모두 충족했다. 이 수치는 실행 시간 차이를 입증하지 않는다.

Periodic, aperiodic, CBS and CBS-wake legacy benches all matched whole ELF SHA-256, entry point, PT_LOAD headers and segment bytes. The changed-load condition activated zero extra runtime controls. layouts.json, identity.json and per-image symbols/disassembly give exact linked inputs. / 기존 벤치 4종 모두 전체 ELF 해시·진입점·적재 세그먼트가 같아 변경 시 추가 실행 조건에 해당한 경우는 0건이다.

[First cold cohort and replay / 최초 콜드 실행과 재검증 원자료](reservation-prepared-grant-first.zip) · SHA-256 673c9e2708ae984c8a3071092bbb53bb5656e3a3f211feeeb95265eef7eb2d45.
