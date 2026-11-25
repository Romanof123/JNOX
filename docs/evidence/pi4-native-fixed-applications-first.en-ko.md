# Raspberry Pi 4 independent verification

Candidate d905d924a90ea1041821bac78919194852263fe5. Prior candidate 001dda0 failed its first FIFO kernel compilation with duplicate .Lpark. Initial host archive CRLF and missing Git metadata failures are preserved. The corrected cohort used the original ordered C and Rust ELF hashes in apps.sha256. A Linux Git checkout produced clean manifests. Upstream QEMU 11.1.3 raspi4b TCG SHA-256 651f3f1b5bb6f388d29f4542f9a3d142f40ab78811807db955903edf81b7a211 ran default reset and clock, four vCPUs, one guest at a time. Firmware boot and physical hardware remain unmeasured.

| First position | Timer rotations | Private C / Rust counts | Exit statuses |
|---|---:|---|---|
| FIFO plain | 0 | 78498 / 70435 | 12 / 77 |
| FIFO traced | 0 | 78498 / 70435 | 12 / 77 |
| RR plain | 16 | 78498 / 70435 | 12 / 77 |
| RR traced | 16 | 78498 / 70435 | 12 / 77 |

CPU0 alone took recorded EL0 exceptions. Each of CPUs 1, 2, 3 ended at model reset park PC 0x310. RR first rotation A to B to A happened before either exit. Independently disassembled workers, saved/restored stack pointer and PC, and IRQ/ERET observations match. CNTFRQ was 62500000. The physical dump covers only live kernel RAM [0x02000000,__stack_top). Each independent TTBR0 page walk resolves the same VA 0x402000 to PA 0x2036000 or 0x244b000, with separate private stack and protected code/data, guard holes and privileged-only devices. Full physical counts and both kernel exit slots were read in all four captures. See inspection.json per attempt for numeric addresses. Scored UART, register monitors, exceptions, argv, and exact images are included.

Virgin virt FIFO and RR with the frozen apps completed 12/77. Existing linked Pi fixed FIFO smoke passed. Both Pi firmware trees passed their pinned checks and raw-image byte comparison. CLI invalid board, policy, count and invalid ELF rejected. Cargo invalid policy, count, and feature rejected. A C source primary and sensor peer exercised separate argument lists, yield and normal exits 17/29. Two 8MiB padded input containers produced a compact 16,816,000-byte raw whose endpoint is 0x1089780 and whose live stack ends at 0x3855000. Its model boot returned 12/77. The ordinary small kernels' ELF PT_LOAD initialized byte ranges were compared against raw bytes with no trailing BSS. A CRLF in the first linked-smoke host wrapper prevented its launch, and the separate Linux command successfully ran its first guest.

The monitor captured terminal state after a fixed four seconds. Raw board reset bypasses firmware execution. Physical power cycling, UART, board timing and product acceptance remain separate.

## 한국어 검증 결과

Raspberry Pi 4에서 동일 우선순위의 일반 C·Rust 프로세스 두 개를 실행하는 경로를 검증했다. 최초 후보 `001dda0`은 FIFO 커널을 빌드할 때 어셈블리 심볼 `.Lpark`를 중복 정의해 부팅 이미지를 생성하지 못했다. 최초 실패 로그를 보존하고, 수정 후보 `d905d92`에서는 앞서 해시를 고정한 앱 ELF 두 개를 그대로 사용했다.

원본 QEMU 11.1.3의 `raspi4b` 모델에서 TCG, 기본 클록 및 기본 멀티코어 리셋 조건으로 정책별 원시 이미지를 직접 부팅했다. 같은 정책의 일반 실행과 예외 추적 실행은 동일한 이미지로 진행했다.

| 최초 부팅 순서 | 타이머 선점 횟수 | C / Rust 계산 결과 | C / Rust 종료 상태 |
|---|---:|---|---|
| 1. FIFO 일반 | 0 | 78,498 / 70,435 | 12 / 77 |
| 2. FIFO 추적 | 0 | 78,498 / 70,435 | 12 / 77 |
| 3. RR 일반 | 16 | 78,498 / 70,435 | 12 / 77 |
| 4. RR 추적 | 16 | 78,498 / 70,435 | 12 / 77 |

FIFO에서는 C 프로세스가 종료한 뒤 Rust 프로세스가 실행됐다. RR에서는 어느 쪽도 종료하기 전에 C → Rust → C 순서로 제어가 넘어갔다. 작업 루프를 역어셈블해 선점 시점의 프로그램 카운터를 확인했고, 상대 프로세스가 실행된 뒤 저장된 명령 위치와 스택 포인터로 복귀한 기록을 대조했다. 예외 추적에서 사용자 모드 인터럽트와 복귀는 CPU0에서 관측됐고, 종료 시점에 나머지 세 코어의 프로그램 카운터는 모두 모델의 대기 위치인 `0x310`이었다.

네 부팅 모두 종료 후 보존한 물리 메모리에서 각 프로세스의 페이지 테이블을 독립적으로 따라갔다. 동일한 결과 변수 주소 `0x402000`이 서로 다른 물리 페이지를 가리켰으며, 그 위치에서 위 계산값 전체를 읽었다. 커널 메모리에서도 두 종료 상태를 확인했다.

이 검증은 QEMU에 원시 이미지를 직접 적재한 결과다. 별도로 준비한 펌웨어 디렉터리 두 개에서는 고정된 파일 해시와 커널 이미지의 바이트 일치를 확인했다. 실제 Pi 4의 전원 인가와 SD 펌웨어 부팅, UART 배선, 보드 타이밍 및 제품 인수는 별도 실물 검증이 필요하다.
