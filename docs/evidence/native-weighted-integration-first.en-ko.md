# Native weighted integration / 네이티브 가중 통합 최초 관측

**3/4 weighted positions passed. The release timing gate failed. 가중 최초 4회 중 3회 통과, 시간 한계 판정 실패.** On the third position (wake/default), lease 14 for A began at 5,296,560 ticks, programmed compare at 5,921,560 and entered the IRQ vector at 6,187,106. Elapsed 890,546 exceeded H=781,250 by 109,296 ticks (1.749 ms at 62.5 MHz). Compare-to-vector was 265,546 ticks. Dispatch gap 487 was within G=156,250. The data localize the excess to the programmed compare–vector interval. The cause is unresolved. No retry or new comparator boot.

세 번째 위치의 A 임대 14에서 비교값부터 IRQ 벡터 진입까지 265,546틱이 걸렸다. 임대 한계를 109,296틱 초과했다. 원인은 미해결이다. 재시도 없이 뒤의 독립 위치를 수행했다.

|Position / 순서|Official replay / 판정|External IRQ/SVC/EL0 returns|
|---|---|---|
|1 share/default|PASS|36/2/38|
|2 share/icount3|PASS|36/2/38|
|3 wake/default|FAIL lease 14|15/11/26|
|4 wake/icount3|PASS|15/11/26|
|Control startup/default|PASS, exit 37|0/2/2|

All four weighted runs retained full raw evidence. Both actual linked C jobs returned via ERET in separate ASIDs, private writable pages and stacks. Replay reconstructed private work/checksums and exits 41/42. Both share runs satisfied every prefix and final bound through 36 IRQ closures. Exit tails are recorded separately in independent.json. Both wake runs completed three absolute 40 ms sleeps, including A's charged work, sleeping B's zero charge, placement, first B return and user-clock sample. All six sets of D/I/W/R/S endpoint tests passed. The separate wake/default lease miss determines its verdict. Startup replay checked original argv, computation, write −38, errno 38, zero app stdout and exit 37.

네 위치의 원본을 모두 회수했다. 독립 주소공간·스택·작업량·종료값 41/42와 반환 경로를 확인했다. 두 분배 위치의 36개 IRQ 및 꼬리 구간을 분리했고, 두 깨움 위치의 총 여섯 번에 대해 최악 표본 끝점까지 검사했다. 단독 앱은 원래 인자와 계산 결과, 미지원 쓰기 오류, 표준출력 0바이트, 종료 37을 충족했다.

Candidate 0ad1eda8fafdf45d948c3761af938cf18e68df80, first parent a941dcad57d5fb2aa7f2853cfa8d7ef0df72d308, second parent f8627e3bdc2023817d7e3f89760e6b05222ba5db, tree 7387c6630e8110883dc8189536b8179addf28689, merge base 7a4ca3bef9a3e789c46dc9810d4f381ba51dacab. Git remerge diff was empty. Main's 1,263 unique trace triples plus the six frozen CHG-088 affects edges exactly matched all 1,269 candidate triples. scheduling_evidence.py returned 1 on main and candidate, with identical four findings (dangling EVD-090, missing tested-in EVD-058, missing Korean and English EVD-058 documents). The native weighted selection is guarded by the native/virt profile. The standalone selection uses native_el0 and weighted uses native_vruntime. The admission configuration inherited from main is preserved by the combined source. Candidate frozen limits and historical receipts merge cleanly.

본선 그래프와 지정한 6개 간선의 합집합은 후보와 일치했다. 감사 도구는 본선·후보 모두 기존 네 지적을 보고하고 종료 1을 반환했다. 전체 소스 변경 및 병합 부모는 combined.diff와 audit.json에 보존했다.

Rust/Cargo 1.95.0, Clang/LLD 14, locked serial release, fresh Linux targets, same checkout/package paths. For all three native applications the complete user ELF equals both rebuilt #183 and its archived user ELF. For all three kernel images, entry, all load headers/payloads and .text match both comparators. Differences reside in nonloaded symbol/string tables. The original three rebuilt #183 kernel files exactly reproduce their archived peers. Main startup changed in loaded code, including SDK clock routines; its candidate first control passed. The five EL1 selectors have identical loaded main/candidate bytes and entries; only nonloaded tables differ. The original EVD-049 verified raw archive and independent verifier passed eight historical positions. Candidate weighted decoder passed eight original UARTs; fixed EVD-022 observer accepted its original 21-choice transcript. Zero fresh EL1 launches. Linked disassembly, section hashes and selectors are included. Weighted entry restores frame, programs CNTP_CVAL_EL0/CNTP_CTL_EL0 and executes ERET; the retained IRQ and exception records identify actual returns.

세 네이티브 앱의 적재 바이트는 #183 원본과 재빌드에 일치한다. 본선 단독 앱과 달라진 후보 적재 코드는 단독 실행으로 확인했다. EL1 다섯 설정은 본선과 후보의 적재 내용이 동일했고, 보관 UART 8건과 고정 벤치 1건을 후보 판독기로 확인했다. EL1 신규 실행은 0회다.

Historical #177 first cohort 0/4 (first IRQ failures, one RAM lost). #183 3/4, default share failed H on leases 7/10 (807801/782806) and prefix 7 (1612999 vs 1562500). #184 first 2/2 launched and both raw sessions lost; second 1/2 launched, plain raw service passed, official replay unavailable, instrumented unlaunched; third 2/2 retained and passed, unchanged binaries. Historical and current failures remain separately counted. No new #184 observer sessions. Results apply to these QEMU observations; repeatability, worst-case delay, physical boards and ASIL qualification remain open.

과거 #177은 첫 IRQ 실패 4건 중 RAM 한 건이 결측이었다. #183은 3/4 통과, 기본 분배 실패였다. #184 세 관측군은 각각 2/2 실행·원본 두 건 소실, 1/2 실행·공식 재생 불가, 2/2 실행·보존·통과였다. 반복성·최악 지연·실보드·ASIL 적격성은 미확인이다.

Each position was sealed to D:, reopened and member-verified by native Windows and WSL Linux, extracted independently, and replayed against its externally pinned session hash before the next launch. Independent archival scorer also checked raw RAM/IRQ/SVC and worst admissible clock endpoints. See external position pins, per-position ZIP members and recorded commands/exit codes. A cross-reader fixture preceded guest launches. The standalone main recipe initially rejected the newer --app argument; the historical original CLI then built successfully before any boot. The machine-readable bundle carries binary identities and historical receipt digests.

각 실행 직후 D:에 ZIP을 봉인하고 Windows 및 Linux에서 독립적으로 다시 열어 모든 크기·해시·재생 결과를 확인한 뒤 다음 위치를 시작했다. 최초 인자 오류는 게스트 시작 전 본선 구식 빌드 도구에서 발생했으며 원래 호출법으로 빌드했다.

[Integrated first cohort, binaries and replay / 통합 최초 실행과 바이너리·재검증](native-weighted-integration-first.zip) · SHA-256 33b2cbe90b60c12691db1ee73ec039e807713b2545b7ecd90bd741509b598dc9.
