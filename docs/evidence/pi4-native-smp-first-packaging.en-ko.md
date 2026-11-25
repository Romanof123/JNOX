# First Pi 4 native SMP package verification / Pi 4 네이티브 SMP 최초 검증

Clean WSL Git checkout 0297172c07053f52241ac65780a998054e5e95d1. Prelaunch freeze includes five SMP positions, four negative controls, legacy Pi FIFO, 120-second timeout, actual #322 low/high ELF hashes and disassemblies, candidate registers W8/W11 in their productive division loops, and matching stock QEMU 11.1.3 raspi4b executable (SHA-256 651f3f1b5bb6f388d29f4542f9a3d142f40ab78811807db955903edf81b7a211). The calendar ELF built successfully via the normal SDK. Builds and guests were serialized.

| Positions | First disposition |
|---|---|
| 1/2 pair plain/traced | Pair packaging failed before generating kernel, manifest or raw image. Both boots unlaunchable. |
| 3/4 remote arrival plain/traced | Remote packaging failed in the same path even with explicit offsets 0,0,1. Both boots unlaunchable. |
| 5 swapped affinities | Swapped package failed in the same path. Boot unlaunchable. |
| 6 legacy FIFO plain | The first and only guest reached terminal. Private C 78498/exit 12 and Rust 70435/exit 77, independent mapped PAs 33775616/38055936, common user VA 0x402000. Replay 0 and no event loss. This acquisition did not sample the secondary park PCs. |

All three first SMP package failures have status 1. The public CLI constructs normalized SMP release cells at tools/build_native_el0.py:380. The later calendar-policy code unconditionally resets cells to an empty list at line 409. During slot assembly line 533 indexes that empty list. Retained directories show copied ordered ELFs and empty build logs. The competing ordinary rejection controls for zero mask, outside mask, fourth slot, and wrong board each stopped before boot with status 2. The fourth-slot diagnostic says “virt supports at most two --peer-elf arguments” even with board pi4. Full commands and logs included. The archived #322 FIFO/RR historical receipts remain independent.

The documented operator entry point cannot supply a bootable SMP image on this head. No claim of Pi 4 SMP scheduling, overlap, remote SGI handoff, frame/TLS isolation or swapped ownership can be made from these firsts. No fallback raw package or reroll. Model-only legacy measurements.

일반 패키지 생성 경로에서 SMP 릴리스 셀 배열을 만든 뒤 다른 정책용 코드가 빈 배열로 덮어썼다. 페어·원격 릴리스·마스크 교환 3개 최초 패키지 모두 슬롯 조립에서 IndexError로 끝나 5개 SMP 게스트를 시작할 수 없었다. 음성 대조군 4개는 부팅 전에 거부되었다. 기존 Pi FIFO만 실제로 한 번 부팅해 전용 메모리의 78498/70435와 종료 12/77을 확인했다. 이 FIFO 실행에서는 보조 코어의 주차 PC를 직접 읽지 않았다. 오류가 난 최초 로그는 보존했다.
