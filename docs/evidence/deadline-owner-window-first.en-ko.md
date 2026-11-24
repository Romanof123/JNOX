# Independent QEMU review of #261 / #261 독립 QEMU 검증

**Commit** 4c62e02a6d6389a0c7885df886870fd5d51124f0, kernel/deadline-window-owner-limit. All eight first guest acquisitions met the prospective terminal and owner-window contract. No new behavioral failure was observed in the sixteen legacy deadline images. All sixteen had changed ELF and loaded bytes.

**판정** 최초 QEMU 게스트 8개 구성은 모두 사전 계약을 충족했다. 기존 deadline 구성 16개의 종결 동작은 유지됐다. 16개 모두 ELF 및 메모리에 적재되는 바이트는 달라졌으며, 실행 코드와 적재 레이아웃은 동일했다.

## Acquisition / 최초 획득

Only the prospective plan and literal operator command were read before running the matrix. Linux x86_64 Rust 1.95.0 on WSL ext4; QEMU virt/GICv2, Cortex-A72, TCG, one vCPU, network disabled. The unmodified POSIX command in recipe.sh ran sequentially, uncached guest outputs in a new evidence directory. All builds, terminal checks and accepted supply replays exited 0. QEMU exited 124 at the specified timeout in all eight positions. The outer recipe exited 0. Acquisition manifests cover build files, ELF, UART, QEMU stderr, exception logs, replay outputs and return codes per position. first-matrix.zip SHA-256 is 86d5543ae2ea66e3de3e6425855977a47fbacea44ce33886cc82df4a2f15cbef. Source, author tests and historical logs were inspected only after sealing.

사전 계획과 운영 명령만 읽고 Linux Rust 1.95.0 및 영속 ext4 경로에서 원문 명령을 실행했다. 빌드·종결 판정·허용 구성 공급 재검증은 모두 종료 코드 0, 게스트 8개는 지정된 타임아웃 124였다. 각 최초 실행의 원본과 해시는 first-matrix.zip에 있다.

## Owners, counter windows and products / 실제 소유자·이동 구간·작업 결과

All four cap-4 runs followed idle → A → service → A → B, then B's SVC parked. Every ERET PC and every interrupted ELR was matched against the linked ELF or emitted frame. The startup ERET targeted the idle symbol, both completion ELRs targeted the post-SVC instruction, and the terminal entry had no ERET. Initial frames matched linked entry addresses and stack tops minus 832; the service cut restored A's exact saved frame and PC. Worker execution counts independently equal sums of exception-entry minus previous-restore timestamps. Both worker deadlines used S+5Q and S+7Q; each worker finished before its deadline. A stopped at S+15,625,000, kept its progress across service and then advanced. Both final products were independently recomputed modulo 2^64 from measured block counts, and service returned the sum of squares 1 through 64, 89,440.

각 허용 실행에서 유휴→A→서비스→A→B 순으로 복원되며 마지막 B의 SVC는 park로 끝났다. 복원·예외 PC, 스택 프레임, 경과 카운터, 절대 마감 및 두 작업의 산술 결과를 대조했다. 최초 유휴 진입과 복원되지 않은 마지막 유휴 선택은 전환 횟수에서 제외했다.

| First guest / 최초 게스트 | IRQ entries | Completion SVCs | Returning changes | Peak entries / changes in Q | Bound |
|---|---:|---:|---:|---:|---:|
| EDF plain cap 4 | 3 | 2 | 4 | 3 / 2 | 4 |
| EDF trace cap 4 | 3 | 2 | 4 | 3 / 2 | 4 |
| LLF plain cap 4 | 3 | 2 | 4 | 3 / 2 | 4 |
| LLF trace cap 4 | 3 | 2 | 4 | 3 / 2 | 4 |
| EDF/LLF plain/trace cap 3, each | 0 | 0 | 0 | No launch / 실행 전 거부 | 4 > 3 |

scores.jsonl gives each first guest’s origin-dependent deadlines, exception timestamps and products. score.py evaluates all distinct window populations by locating the integer starts at which each event enters or leaves [a,a+Q), including both clamped endpoints. The observation ends at S+17Q. Every timer arrived in its own programmed cell. Both SVCs preceded the next compare. All guest per-event and terminal counts agreed with independent calculations.

슬라이딩 구간 안의 이벤트 수가 바뀌는 모든 정수 시작점과 양 끝점을 검사했다. 모든 IRQ는 지정된 셀에 도착했고 두 SVC는 다음 비교 시각 전이었다. 행별·최종 집계가 모두 일치했다.

## Supplemental evidence / 별도 측정

Eight independent QMP snapshots used the first matrix ELFs. The files under ram/ are new guest runs with their own UART and exceptions. Cap 3 left all 81,920 bytes of the five stacks, all 24 rows, row count, restore stamp, worker progress, service sum and service SP zero. All original refusal interrupt logs were empty, and source places refusal before frame initialization and timer arming. In cap 4 snapshots the five in-memory 448-byte rows matched every D and J field. The final restore stamp equaled the terminal entry's previous-restore field, and worker and service memory agreed with the transcript.

동일한 최초 ELF로 별도 실행하여 QMP로 RAM을 수집했다. 거부 실행은 스택 전체·행·복원 타임스탬프·진행량·서비스 측정값이 0이었다. 허용 실행은 RAM 행 전체와 D/J 원본이 일치했다. 별도 실행의 카운터는 최초 실행과 구분해 보존했다.

## Host oracle and applicability / 호스트 검증 및 적용 범위

The independent oracle called the production Rust helper using a minimal executable. Across Q=1..4, cells=1..3 and 0..2 completions, 14,648 placements and 953,766 complete half-open windows were enumerated, including endpoint coincidences, adjacent IRQs and early completions. Eight invalid/extreme checked-arithmetic cases agreed. Another 11,200 clipped-window comparisons covered repeated owners, worker/service/idle transitions and final park. All eight tests in the authored supply suite passed. These results establish a conditional bound for the finite single-CPU model assuming each timer enters its programmed cell, each admitted job completes at most once and each dispatch returns at most one new owner. Four is the admission bound; the measured peak of actual transfers was two. Tests cover emulated architectural ticks; physical timing and unrestricted workloads remain outside their scope.

실제 커널 함수를 호출해 IRQ 오프셋·조기 완료 배치 14,648개 및 완전한 구간 953,766개를 전수 열거했다. 범위·오버플로·끝점과 동일 소유자·park 또한 검증했다. 상한 4는 유한 단일 CPU 및 계약의 IRQ·완료 조건에 따른다. 게스트에서 실제 관측된 최대 전환은 2였다. 실물 보드나 일반 부하의 초당 보장은 측정하지 않았다.

## Legacy comparison / 기존 구성 비교

The historical 28-ELF archive SHA-256 was independently verified as e6b226c7e8f8d430bbe0167b437acbb7626d771eb090f28e2f7f71971861f20e. All sixteen deadline profiles were rebuilt, run, and compared with the historical ELF bytes. Twelve finished, four tight-supply controls refused before launch, and eight legacy supply replays passed. No legacy UART contained an OWNER_ record. For each image, PT_LOAD headers and all executable bytes matched. Every differing loaded byte, 29–38 per ELF, was decoded as a Rust .rodata source-location line number with identical filename pointer, length and column; no other loaded bytes differed. legacy/sections.json and locations.json preserve the per-image details. The twelve historical multilevel images were outside this deadline-only comparison.

기존 28개 ELF 보관 파일의 SHA-256을 확인한 뒤 deadline 16개를 각각 재빌드·부팅했다. 12개 완료, 4개 공급 거부, 공급 관련 재검증 8개 성공이었다. 각 ELF의 적재 헤더와 실행 코드 바이트는 동일했다. 바뀐 적재 바이트는 ELF당 29~38개였으며 모두 .rodata에 있는 Rust 소스 위치의 행 번호로 해석됐다. 기존 multilevel 12개는 이 비교 범위 밖이다.

[First owner-window QEMU matrix, supplementary raw RAM, and exhaustive host checks](deadline-owner-window-first.zip) · SHA-256 48fc75bd6da5295340903efb9815cefb59667ba278fd62250d05a344c59e1706.
