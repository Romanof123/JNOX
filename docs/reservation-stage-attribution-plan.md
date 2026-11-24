# CHG-071 · First reservation IRQ attribution / 첫 예약 IRQ 구간 계측

Proposed experiment on experimental head `61a439999e5cb8ec991ad7b533b2bf6ca7b68f60`. REQ-002/005/009/023, DES-003/012, VER-002/005/023. The decision concerns the next source investigation. Full-profile qualification and P1/deployment remain open. [#123](https://github.com/Romanof123/JNOX/issues/123) · [Reservation contract](pol-008-rust-reservations.md) · [Compact captures](evidence/evd-053-v3.en.md) · [Compiler captures](evidence/evd-053-compiler.en.md)

REQ-009 covers the footprint comparison. [DES-003](architecture.en.md#des-003-threads-and-real-time-scheduling) · [DES-012](architecture.en.md#des-012-scheduling-policy-layers) · [설계 원문](architecture.md) · [VER-002/005/023](verification-plan.en.md) · [검증 계획](verification-plan.md)

Replay inputs include the [pinned corrected decoder](https://github.com/Romanof123/JNOX/blob/54218be8e6e06382cd491bc2e3af9f61705d4f0c/tools/reservation_handoff.py), the [compact corpus](evidence/evd-053-v3-captures.zip) and [digest](evidence/evd-053-v3.json), and the [compiler corpus](evidence/evd-053-compiler.zip) and [digest](evidence/evd-053-compiler.json). [Compact report](evidence/evd-053-v3.en.md) · [압축 인계 보고서](evidence/evd-053-v3.md) · [Compiler report](evidence/evd-053-compiler.en.md) · [컴파일러 보고서](evidence/evd-053-compiler.md)

## Question and source

Which half of a locally split first-opening interval consumes most of its observed time? Compact v3 measured 15,522–22,627 ticks between the end of source reads and the precommit sample across eight ordinary launches. In the compiler study the size arm completed 1/8, O2 completed 0/8 and no pair had two first markers. Use the stock release opt-level=s. At b9a247c, kernel/src/ordinary_reservations.rs lines 1011–1070 put the source timestamp before receipt stores, live timer checks, disable, barriers and GIC EOI. Lines 680–712 check source ledger, idle return, both worker frames and the destination before precommit. Lines 545–569 give the frame/progress checks. Lines 392 onward already format compact evidence at termination. In kernel/src/reservation_jobs.rs lines 239–245 the configuration comparison covers manifest and backend discriminants. Existing brackets cannot price that comparison.

## Capture contract

1. Freeze two ordinary selectors. Aligned-W/plain completed with marker 59,769 in v3 and stopped at mask 7 in the compiler size arm. CBS-S/trace completed with marker 60,616 in v3 and 58,439 in the compiler size arm and supplies external exception evidence. Keep the strata separate. Use B (baseline), E (EOI), F (frames). Freeze twelve launches in four adjacent triplets in this order: W B/E/F, S F/E/B, S B/E/F, W F/E/B. Each ordinal receives one fresh default-clock guest. Record every first attempt including missing/partial data. No substitutions, warm-up, injection, artificial wait or runtime enable operation.
2. E takes exactly one additional raw architectural counter sample immediately after the final EOI DSB on the first compact opening. F takes one after successful validation of the second live frame and before destination-bank access. Preserve order in the linked instructions. Record the reached site and raw tick in bounded diagnostic storage and format only on termination. A distinct presence indication must distinguish zero from absent. Preserve original source reads, receipt timestamps and validity bits, all checks, epoch, actual failure operands, 62,500-tick opening, publication, IRQ acknowledgement, cutoff, assembly return and independent worker marker. Never derive an endpoint from a sealed expectation. Link the diagnostic wire record to schema, site, selector, stage and boot identity. B has no diagnostic record.
3. Before launch pin source/patch, compiler/linker, flags, selectors, maps and hashes of each executed ELF, decoder, independent reader, QEMU and launcher. Follow CHG-064 stock toolchain/argv, same host, native filesystem, serialized 15 s launches. Keep trace options identical within each S triplet. Record host load, UTC, ordinal, actual argv and status. Match each B load image and executed path bytes/addresses from vectors through worker entry against a separately built untouched parent for its selector. Check against the archived stock reference and reconcile provenance. Reject baseline drift and any extra baseline branch, store, counter read, call or changed layout. Disassemble each E/F ELF to verify site, exact sample count on the opening, all guards and endpoint ordering. Archive code/data/BSS/stack reserve deltas. A static map leaves high-water and physical latency unknown.
4. Independent replay reads original UART, per-ELF maps and S exception logs. Validate framing, role and multiplicity (including both tag-24 roles), endpoint presence and monotonicity. For each E or F run compute probe−source_end and pre−probe only when both operands were recorded in that boot. Verify their sum equals pre−source_end where complete. Never subtract an E sample from an F sample or impute a missing endpoint. Prefixes include measurement overhead. Report IRQ−epoch, source_end−IRQ, whole bracket, commit, arm, selection, actual assembly return and first Rust marker, by ordinal. For every guard report passed when established, first failed with raw status/reason/slot/owner/two operands, or unobserved. Report mask, terminal framing, bank/readback, grants and observed progress, including earlier stops and missing terminals. A 2/5 guard and a 2/2 worker miss retain their original meanings.
5. Check healthy paths with an independent ledger/recurrence replay, stack and saved PC, actual assembly returns, cutoff/readbacks and upper service accumulated per owner/deadline. W success requires the documented 34-slot horizon and S the 10-slot horizon with four A and four B grants. Join S external IRQ/SVC/ERET order to the actual ELF. Validate the reader on retained healthy and early-stop captures and reject offline copies carrying wrong site/hash, absent or duplicated records or impossible order. Preserve originals, parsed outputs, failures and manifests by digest in two verified copies.
6. For each selector/site, label a side consistently larger only when both diagnostic launches reach both ends and the same side exceeds half the local total in both. Publish both fractions and margins. Compare B and diagnostic whole-bracket ranges, marker coverage, terminal status and guard reachability within each triplet as observer-effect evidence. Treat changed reachability or nonoverlapping whole-bracket ranges across the two repeats as grounds to withhold transfer to B. Incomplete/inconsistent data or mixed directions yield an inconclusive result for that site. A repeatable split nominates only a region for further study. Qualification, physical timing, isolation and P1 decisions require separate evidence.

## 한국어 규범

1. 기존 8회에서 소스 판독 종료부터 커밋 직전까지 15,522–22,627틱이 걸렸다. 위 두 실측 사례로 범위를 고정한다. Aligned-W/plain은 완료와 마스크 7 중단을 모두 보였고 CBS-S/trace는 두 연구에서 완료되어 외부 예외 기록과 연결할 수 있다. 위 표기 순서대로 4개 묶음, 총 12회만 실행한다. 기본 시계, 기존 크기 최적화, 1 ms 한계, 첫 부팅을 유지하고 모든 실패·누락을 보존한다. 예열·교체 실행·고장 주입·인위적 지연·실행 중 활성화는 제외한다.
2. E는 첫 compact IRQ의 최종 EOI 장벽 직후, F는 두 번째 프레임 검증 직후 목적지 원장 접근 전에 각각 카운터를 한 번만 읽는다. 유효 표시와 원시 시각을 제한된 저장소에 기록하고 종료 시 출력한다. B에는 추가 코드나 관측 레코드가 없어야 한다. 실제 프레임·신원·진척·원장·시계·IRQ 검사, 타이머 읽기값, 독립적인 어셈블리 복귀와 Rust 표식, 원래 실패 피연산자를 모두 유지한다.
3. 실행 전에 소스, 설정, 컴파일러와 링커, 각 ELF·맵, 관측 및 독립 판독 도구, QEMU와 실행 명령의 신원을 해시로 고정한다. 같은 호스트에서 15초씩 직렬 실행하고 S의 외부 추적 옵션을 일치시킨다. B와 원본의 적재 이미지 및 벡터부터 작업자 진입까지의 기계어·주소를 대조한다. 원본 경로에 추가 명령이나 배치 차이가 있으면 중단한다. E/F의 단일 관측 위치, 검사 순서, 메모리 증감을 ELF에서 확인한다.
4. 독립 판독기는 원본 UART와 ELF, S의 외부 로그에서 실행별 두 구간과 합계를 복원한다. 같은 부팅에서 양 끝이 관측된 차이만 쓴다. 실행 간 차감이나 미관측값 대체를 금한다. 매 실행의 단계 비트, 통과·최초 실패·미관측 검사, 실제 실패 코드·슬롯·소유자·피연산자, 커밋·반환·작업자 시각 및 진척을 남긴다. 원본의 2/5와 2/2 구별을 유지한다.
5. 정상 W 34슬롯과 S 10슬롯(A/B 각 4회)을 원장, 스택·PC, 어셈블리 복귀, 갱신 점화식과 할당별 누적 서비스로 독립 확인한다. 보존된 정상·조기 중단 자료로 판독기를 확인하고 잘못된 해시·위치·순서·누락·중복 사본을 거부한다. 원본과 결과는 해시를 확인한 두 사본으로 보존한다.
6. 동일 사례·관측점의 두 실행에서 양 끝이 모두 보이고 같은 쪽이 각 전체 구간의 절반을 초과해야 그 구간을 후속 조사 대상으로 삼는다. 각 비율과 차이, B 대비 관측 영향과 도달 단계를 공개한다. 도달 단계가 달라지거나 반복 측정에서 전체 구간 범위가 겹치지 않으면 B에 적용하지 않는다. 누락·상충은 판정 불가로 남긴다. 이 결과만으로 P1, 물리 시간, 격리 또는 배포 적격성을 판정할 수 없다.
## Diagnostic wire and replay / 진단 레코드와 재생

CHG-071 implements an experimental observation for IMP-053 under the existing REQ/DES/VER links above. At build time leave JNOX_RESERVATION_STAGE unset for B, or select eoi (E) or frames (F) for a planned diagnostic. RAW4 adds one terminal row before the compact dump. Its eight unsigned words are tag 38, raw tick, slot 0, owner 2 (idle), site (E=1/F=2), selector (aligned-wake=1/cbs-saturated=4), presence (0/1), and the address of the executed MRS CNTPCT_EL0. A zero tick is a valid observation when present. Absence encodes zero tick and PC. Baseline emits RAW3. All ordinary receipt and terminal rows retain their original meanings.

Before launch bind ordinal, selector, site, elf_sha256 and counter_pc (null for B) to the frozen manifest and per-ELF disassembly. Freeze source and tool identities separately. After the first launch add uart_sha256 from immutable capture metadata. Replay each launch with `python tools/reservation_stage.py LAUNCH.json UART ELF`. The reader checks ordinal assignment, digests, executable segment, instruction, row identity, same-boot ordering and legacy compact receipt. It returns the full terminal row and observed intervals. Use the separately pinned independent audit for the complete ledger, exception order, worker and horizon checks. Whole-program disassembly, baseline equivalence and original launch evidence are execution deliverables.

B는 환경 변수를 설정하지 않는다. E/F는 빌드 시 각각 eoi/frames를 선택한다. RAW4 태그 38은 원시 시각, 슬롯 0, 유휴 소유자 2, 위치 번호, 사례 번호, 관측 여부, 실제 카운터 명령 주소를 담는다. 관측 여부가 0이면 시각과 주소도 0이다. 1이면 시각 0도 실측값이다. 실행 전에 순번·사례·위치·ELF 해시·명령 주소를 고정하고 최초 UART의 해시를 기록한다. 위 명령으로 각 부팅의 구간과 실패 피연산자를 복원한다. 원장·예외·실제 작업자·전체 구간은 별도 독립 판독으로 확인한다.

The aligned-wake baseline failed the required load-image identity check. Zero of the twelve planned guests launched. [First reservation stage preflight](evidence/reservation-stage-first.en.md)

정렬 깨움 기준 이미지가 필수 로드 이미지 동일성 검사를 통과하지 못했다. 예정된 게스트 12건 중 실행은 0건이다. [첫 예약 단계 사전 검사](evidence/reservation-stage-first.md)

## First preflight and correction / 최초 사전 비교 및 수정

[First preflight evidence](evidence/reservation-stage-first.en.md) · [최초 사전 검사 증거](evidence/reservation-stage-first.md)

The first aligned-W B load-image comparison stopped before any guest launch. The historical and freshly built parent ELF both hash to 7cfa99d34389ee71a8e55bc65e1e89dfd75f2e4abcdea017ffef51132e89fbb4. Candidate B hashes to f6ae56af318136323ef5d7d8356719bed5d5d34c959533f2b8e3cd2597876a4e. Both executable sections contain 23,864 identical bytes at 0x40200000. The writable load segments also match. The read-only load segment differs at ten bytes. The original baseline.txt, loads.txt, maps, disassemblies, ELFs and SHA256SUMS belong to the rejected first preflight. Preserve them separately from subsequent comparisons.

The retained section tables contain no DWARF. Decoding the 24-byte little-endian filename-pointer, filename-length, line and column records at each mismatch identifies Rust source locations. All ten filename slices equal src\ordinary_reservations.rs, and all ten columns match. For example, the parent disassembly at 0x40202674 addresses the record at 0x40205e00, whose line word begins at 0x40205e10.

| Address of changed byte | Parent → candidate line | Column |
|---|---|---:|
| 0x40205e10 | 146 → 151 | 17 |
| 0x40205e40 | 741 → 748 | 24 |
| 0x40205e58 | 599 → 604 | 13 |
| 0x40205e70 | 855 → 862 | 16 |
| 0x40205e88 | 909 → 916 | 21 |
| 0x40205eb8 | 286 → 291 | 13 |
| 0x40205ed0 | 296 → 301 | 9 |
| 0x40205ee8 | 238 → 243 | 19 |
| 0x40205f00 | 259 → 264 | 9 |
| 0x40205ff8 | 614 → 619 | 26 |

Keep the original module filename and line layout. Wrap four existing expressions in compile-time macros whose baseline expansions reproduce the original expressions. Keep the E/F-only code in a separate file. Record the hook source and each actual ELF. Repeat the complete frozen baseline and instruction checks for both selectors in a new evidence directory before any guest. A missing or changed load byte stops the study.

첫 aligned-W B 준비에서 실행 코드와 주소는 일치했으나 읽기 전용 적재 영역 10바이트가 달라 게스트 실행 전에 중단했다. 보존 ELF에는 DWARF가 없으며, 위 주소에서 파일명·길이·행·열을 해독해 같은 파일의 행 번호만 달라진 것을 확인했다. 최초 거부 ELF, 해시, 맵 및 비교 로그는 원본으로 보존한다. 원래 소스 파일 경로와 행 배치를 복원한다. 기존 표현식 네 곳을 매크로로 감싸고 B에서는 원래 표현식으로 확장한다. E/F 전용 코드는 별도 파일에 둔다. 두 선택자의 전체 적재 바이트와 실제 명령 검증을 새 디렉터리에서 다시 수행한다. 최초 시도와 수정 결과는 별도 기록한다.

Both baseline load images matched. Both E images failed the required linked instruction order. Zero of twelve planned guests launched. [Second reservation stage preflight](evidence/reservation-stage-v2-first.en.md)

두 기준 로드 이미지가 일치했지만 E 이미지 둘 모두 링크된 명령 순서 조건에 실패했다. 예정된 게스트 12건의 실행 수는 0이다. [예약 단계 두 번째 사전 검사](evidence/reservation-stage-v2-first.md)

## CHG-071 revision 3 · EOI sample placement / EOI 관측 위치

[Original v2 static preflight](evidence/reservation-stage-v2-first.en.md) · [v2 최초 사전 검사](evidence/reservation-stage-v2-first.md)

Preflight v2 at 45d21d76d924ca3cc1c6248c3f7dbddddecd72e0 matched both B load images to the fresh and historical references. In both E ELFs the final EOI DSB SY at 0x402013e4 precedes a CBNZ at 0x402013e8, ISB at 0x402013ec and MRS CNTPCT_EL0 at 0x402013f0. The intervening decision failed the frozen E-site criterion. All twelve positions retain zero launches and absent UART, terminal and timing evidence. Keep the first load-byte refusal and this second static refusal as distinct original records. Trace: CHG-071 · REQ-002/005/009/023 · DES-003/012 · VER-002/005/023.

For E, decide whether this is the first compact opening before the sampled barrier. On the chosen path issue the required final EOI DSB SY, ISB and MRS CNTPCT_EL0 in that order in one opaque asm block. Recover the MRS address in the same block. On every other E path issue the original completion barrier. Preserve RAW4, its PC witness, validity and actual guard operands. Keep B's source file, line positions, default expression tokens and RAW3 intact.

For each new E ELF, follow the full IAR EOI store and both branch successors in the linked image. Require exactly one completion DSB on either path. The sampled successor must contain adjacent DSB/ISB/MRS instructions and the ADR must point to that MRS. For each F ELF trace both iterations through the live frame checks and each failure edge. The only successful second-frame exit must reach the F ISB/MRS before any destination-bank access. In the retained v2 F images the final loop test at 0x40201858/5c branches directly to 0x40201968, with MRS at 0x4020196c. The preceding linear address 0x40201964 calls the nonreturning failure path. Verify the actual control-flow edges anew for each ELF. Baseline load identity, original endpoints, guards, frozen case order and 1 ms limit remain prerequisites for launch.

v2 사전 검사에서는 두 B 이미지가 기준 적재 바이트와 일치했다. 두 E 이미지 모두 최종 DSB와 ISB 사이에 CBNZ가 있어 중단했고 12회 중 실제 실행은 0회다. 최초 B 불일치 기록과 별개로 보존한다. E에서는 첫 개방 여부를 먼저 결정하고 해당 경로에서만 최종 DSB SY·ISB·CNTPCT 판독을 한 asm 블록으로 묶는다. 다른 경로의 완료 장벽과 RAW4 위치 증거를 유지한다. 새 ELF별로 실제 IAR EOI 쓰기에서 양쪽 경로를 따라 장벽 수와 연속 명령 및 ADR 주소를 확인한다. F는 두 프레임의 모든 실시간 검사와 실패 경로, 성공 분기에서 목적지 원장 접근 전 관측까지를 링크된 제어 흐름에서 검증한다. B 원본 비교, 기존 1 ms 검사 및 동결 순서는 그대로 적용한다.

Aligned wake (W, plain) completed 3/6 first boots. Saturated CBS (S, traced) completed 2/6. No baseline boot reached its horizon. The opening guard remained 62,500 ticks (1 ms). [First v3 reservation stage captures](evidence/reservation-stage-v3-first.en.md)

정렬 깨움(W, 일반) 6건 중 3건, 포화 CBS(S, 외부 추적) 6건 중 2건이 완료됐다. 기준 이미지 B는 완료된 부팅이 없었다. 경계 한도는 62,500틱(1 ms)으로 유지했다. [예약 단계 v3 최초 실행](evidence/reservation-stage-v3-first.md)

### CHG-081 · sharing checked work on the decision path

The first v3 cohort completed 3/6 aligned wake and 2/6 saturated CBS boots across B/E/F. Neither baseline reached the horizon; the split observations do not identify the dominant baseline cost. Keep the first results. [First v3 evidence](evidence/reservation-stage-v3-first.en.md)

1. Validate both live initial frames during idle preparation and again during masked slot-zero opening. Compare each actual pointer with its prepared address and canonical top-of-stack minus the complete frame size. Prove at compile time that the frame fits the fixed stack and its alignment. Exact equality with that canonical address then also proves containment. Keep live register, thread, state, progress and allocation checks. Leave the F sample after both successful checks.
2. At each subsequent masked boundary, read both client progress records once and validate them against the last retired values. A successful comparison permits the selected grant to use its validated baseline count/checksum. No worker, IRQ or asynchronous device writes the cells between verification and dispatch. Keep the independent retirement read, both final observations, and the original first-worker, pre/post authority, cutoff, selection and expiry checks. Both authoritative bank reads and actual ERET/IRQ records remain independently observable.
3. Freeze matched parent/candidate images and first cold default-clock positions for both original strata, B/E/F in both repeats. Compare linked code size, loads/branches and actual phase intervals before interpreting any change. Include aligned/CBS refusal and late controls, validate the full slot horizon and independent return/progress ledgers. The original 1 ms rule and fail-stop dispositions apply. This is a source refactor with no measured speedup or product allocation. [REQ-002/005/009/023](requirements.en.md) · [VER-002/005/009/023](verification-plan.en.md)

### CHG-081 · 결정 경로에서 검증된 값 재사용

최초 v3 코호트의 B/E/F 전체에서 aligned wake는 3/6, saturated CBS는 2/6 완료했다. 기준 B는 지평선에 도달하지 못했다. 구간 분할 결과로 기준 경로의 지배적 비용을 특정할 수 없다. 최초 판정을 보존한다. [최초 v3 근거](evidence/reservation-stage-v3-first.md)

1. 유휴 준비와 IRQ가 차단된 슬롯 0 개시 때 각각 두 실제 프레임을 검사한다. 실제 포인터를 사전 기록 주소 및 전용 스택 끝에서 전체 프레임 크기를 뺀 주소와 비교한다. 고정 스택에 프레임 크기·정렬이 맞음을 컴파일 시 보장하면 이 주소의 일치가 범위 포함도 증명한다. 실제 레지스터·스레드·상태·진행·할당 검사를 유지한다. F 표본은 두 검사 성공 뒤에 둔다.
2. 이후 차단된 경계에서는 양쪽 진행값을 한 번씩 읽어 직전 반납값과 확인한다. 일치하면 선택된 작업의 부여에 검증된 개수·체크섬을 재사용한다. 검증과 디스패치 사이에 작업·IRQ·비동기 장치가 해당 셀을 쓰지 않는다. 반납 시 독립 판독, 양쪽 최종 관찰, 최초 작업 표본, 권한 전후 검사, 차단·선택·만료 조건을 유지한다. 두 권한 판독과 실제 ERET·IRQ는 별도 관찰 가능해야 한다.
3. 부모/후보 이미지와 두 기존 층 각각 B/E/F 두 반복의 기본 클록 최초 콜드 위치를 고정한다. 링크 크기·읽기·분기와 실제 구간을 비교한 뒤 해석한다. aligned/CBS의 거부·지연 대조, 전체 슬롯 지평선, 독립 복귀·진행 기록을 확인한다. 기존 1 ms 기준과 중단 판정을 적용한다. 소스 정리 단계이며 속도 개선·제품 시간 할당은 미측정이다. [REQ-002/005/009/023](requirements.md) · [VER-002/005/009/023](verification-plan.md)
