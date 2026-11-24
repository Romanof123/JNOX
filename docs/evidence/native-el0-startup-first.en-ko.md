# First native AArch64 EL0 process / 첫 AArch64 네이티브 EL0 프로세스

The first cold guest at 59b006f5c57d607c49bc8331f8446828474e5d33 executed the unmodified SDK v1 C startup probe in EL0 and exited with its computed status 37. The original host replay returned 1 because it omitted the two ordinary privileged boot banners from its strict UART expectation. Preserve that original result and capture.

첫 콜드 게스트는 수정하지 않은 SDK v1 C 시작 프로브를 EL0에서 실행하고 계산 결과 37로 종료했다. 최초 호스트 재생기는 커널의 기존 부팅 배너 두 줄을 허용하지 않아 코드 1을 반환했다. 최초 결과와 캡처를 보존했다.

| Same-process evidence / 동일 프로세스 근거 | Observed / 관측값 |
|---|---|
| Native application SHA-256 | f80ce35a07cb8a3275313ce74d114dbb782d19eed4624aa0b115134e0d88b195 |
| One cold launch / 최초 실행 | QEMU 6.2 virt, Cortex-A72, TCG, GICv2, one vCPU, 128 MiB |
| Architectural entry / 실제 진입 | EL1→EL0 ERET at 0x400000 |
| Initial SP and argc / 시작 스택과 인자 수 | 0x1010fb0, 16-byte aligned, argc=3 |
| Stack argv / 인자 | /bin/native_startup_probe, 8, 13; argv[3]=NULL, empty environment |
| Actual page-table permissions / 실제 페이지 권한 | RX text at 0x400000, RO/NX at 0x401000, RW/NX at 0x402000, sixteen RW/NX stack pages, invalid guards at 0x1000000 and 0x1011000 |
| First trap / 첫 예외 | EL0t, lower-A64 vector 0x40201400, SVC #0x4a01, full ESR 0x56004a01, saved ELR 0x4000c8 |
| Write / 쓰기 | syscall 4, fd=1, length=3, user bytes 33 37 0a. Raw return −38, C return −1 and errno=38 (ENOSYS). ERET to EL0 at 0x4000c8 |
| Computation / 계산 | 8×2 + 13×1 + 8 = 37; stopped user products [16,13], weights [2,1], bias=8 |
| Second trap / 두 번째 예외 | Same EL0t vector and syndrome, saved ELR 0x400020, syscall 1, x0=37, exit disposition |
| Application stdout / 앱 표준 출력 | 0 bytes |
| Privileged UART / 커널 UART | JNOX M1 BOOT, JNOX M1 READY, NATIVE EL0 ELF followed by the app digest, NATIVE EL0 EXIT 37 |
| Immutable original session / 원본 세션 SHA-256 | 849a6877ffbf20a8ff96939603b2e0fca59d00432496a49cbbb2f0fc940f1976 |

The original build used native Linux Rust/Cargo 1.95.0 with Ubuntu clang and LLD 14, one job, on WSL ext4. The kernel ELF digest was ee48323195b4bce120e7e0e0d614094c8927d72463b640a7feba3e95e3fce165. The host observer stopped the original QEMU process and captured RAM. Walking its page tables recovered argv, actual user page permissions and inaccessible guards. The same-process exception log independently supplied the entry and both trap/return addresses. The application checks its zero-initialized products before multiplication. Stopped memory contains the final products; there is no independent pre-entry RAM snapshot.

원본은 WSL ext4에서 Linux Rust/Cargo 1.95.0 및 clang/LLD 14로 단일 작업 빌드했다. 같은 QEMU 프로세스를 정지하고 얻은 RAM에서 실제 페이지 테이블, 스택 인자, 가드와 최종 산술 결과를 확인했다. 외부 예외 추적에서 진입 주소와 두 예외를 대조했다. 앱은 곱셈 전 BSS 초기값을 검사했고, 캡처한 메모리는 종료 상태의 표본이다.

This receipt covers one native application on an emulated AArch64 target. It supplies no general POSIX I/O, physical-board, reference-product compatibility or deployment qualification. / 단일 에뮬레이터 대상의 시작·미지원 쓰기·종료 관측이며 일반 POSIX I/O, 실보드, 참조 제품 호환성 또는 배포 적격성은 별도다.

The host-only revision a7451e3fb256ed94b7937ba68361caeb6961e476 returned **0 on the same immutable first session**. Its UART control accepted the exact four-line stream and rejected ten alterations, including inserted application output. Original host verdict **1**, revised host verdict **0**, guest count **1**.

호스트 전용 수정판은 **동일한 최초 세션에서 코드 0**을 반환했다. 정확한 네 줄을 허용하고 앱 출력 삽입을 포함한 열 가지 변경을 거부했다. 최초 호스트 판정 **1**, 수정판 판정 **0**, 게스트 실행 **1회**다.

[Metrics / 측정값](native-el0-startup-first.json) · [Reproducible first capture / 최초 원본 자료](native-el0-startup-first.zip) · [Frozen plan / 고정 설계](../native-el0-startup.en.md)

Signed by role / 서명 역할 — independent execution and verification reviewer / 독립 실행·검증 담당.
