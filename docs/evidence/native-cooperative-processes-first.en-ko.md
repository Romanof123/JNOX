# Independent application verification / 독립 애플리케이션 검증

Romanof123/JNOX #291/#292, commit `94bef763dbbcc7419e2d2902361052c6acec1f86`. All scored runs used one QEMU virt Cortex-A72 vCPU under TCG. No hardware or timing-performance claim. Six serial workloads plus one read-only post-exit observation. Results and exact guest commands appear under `apps-serial/`.

## English

| Workload | Guest observation |
|---|---|
| Shipped distinct C/Rust processes | Dispatch 0→1→0. C sleeps, Rust updates its private word and exits 29. C resumes, checks its deadline and word 13, yields with no surviving peer, then writes and exits 17. |
| Running Rust peer | C sleeps 100 ms. Rust computes a rolling checksum and reads the monotonic clock until its own 300 ms deadline. An IRQ interrupted EL0 and returned to the same PC 0x4016d0. Rust voluntarily yielded, C yielded back, Rust exited 29, then C exited 17. |
| Two sleepers | Both sleep. Two IRQs originated at idle EL1 PC 0x402013f4. C completed its earlier deadline and exited 17 before Rust completed the later deadline and exited 29. |
| Single startup, clock and sleep | Startup exit 37 requires argv/BSS/data and write returning -1 with errno ENOSYS. Clock checksum exited 0. Three-round absolute sleep exited 0 after checking deadlines, already-due return, invalid timespec and errno/remaining preservation. |

The shipped ELF symbol tables put both 4-byte writable accumulators at VA 0x402000, initialized respectively to 10 and 20. The ELF bytes and read-only monitor output are under `apps-serial/pair/`. In the shipped kernel, SPACES begins at 0x40226000. Space has five table pages preceding image storage, a 4 MiB image and 64 KiB stack, hence stride 0x415000. For the target VA, the two image-page descriptors at 0x40228010 and 0x4063d010 resolve to physical 0x4022d000 and 0x40642000. Post-exit words are 0x11 and 0x1d. The separate inspection boot has a matching complete UART. The observer issued only QEMU monitor reads (`xp /1gx` for each descriptor, `xp /1wx` for each word), then quit.

The executable tests cover each application's continuation, stack use and writable state. The vector and Rust frame sources show GPR, SP, ELR/SPSR, SIMD/FP and TLS save/restore. The ordinary workloads do not put distinguishable live test values into every register. IRQ traces report the exception level and PC. The cooperative application code and UART provide the ownership and voluntary-yield sequence.

## 한국어

채점한 6개 워크로드는 한 번에 게스트 하나씩 실행했다. 배포 C와 Rust는 같은 가상 주소 0x402000에 각각 10과 20을 저장한 상태로 시작한다. 두 번의 자발적 양보 뒤 C가 대기하자 Rust가 값을 갱신하고 29로 종료했다. C는 자기 값 13과 기한 도달을 확인했고, 상대가 종료한 상태에서 다시 양보한 뒤 17로 종료했다. 별도 부팅에서 종료 후 읽은 실제 물리 페이지 0x4022d000과 0x40642000에는 각각 17과 29가 남아 있었다.

Rust가 체크섬을 계산하는 경우 IRQ가 EL0에서 발생했고 예외 복귀 PC는 중단 PC 0x4016d0와 같았다. Rust가 스스로 양보한 후 C가 실행됐다. 두 프로세스가 모두 대기한 경우 유휴 EL1에서 IRQ가 두 번 발생했고 100 ms 기한의 C가 먼저, 300 ms 기한의 Rust가 나중에 종료했다. 단일 프로그램에서는 시작 규약과 미지원 write의 ENOSYS를 확인한 뒤 37로 종료했고, 단조 시계와 절대 시각 수면 예제는 각각 0으로 종료했다. 모든 레지스터에 개별 시험 값을 넣어 동적으로 검증한 것은 아니다. 실제 하드웨어와 성능은 측정하지 않았다.

## Provenance / 실행 기록

1. Historical temporary cohort, witness only. Original WSL scratch `/tmp/jnox-292-apps.JdbAZSo3` later disappeared. The cause was not independently established. Original first manual launch lacked `-nodefaults` and stopped at QEMU's missing efi-virtio.rom before guest execution. The unchanged packaged pair passed with the project's ordinary QEMU arguments. The original pair kernel/primary/peer hashes were ef9faaf9992ad5d02e4751640ae91515dbef3b69ab5440aeb4bf99ac6184bc98, 7fb056886d6329ef6d843e34d549d6f1ca1871aed42a6983e258a2d997fcdcb6 and 2611fda1ca646010ca6cf8789386b853dce4efb4c2ecca6cdef2e4cc2d12f790. Both original supplemental peers exited 112 at a retained fixture assertion because the larger applications linked their data at 0x403000. Removing the assertion from separate supplemental inputs admitted both timer paths. Their observed running IRQ PC was 0x40018c. Original binaries and logs are unavailable.
2. Secondary recovery cohort, unscored. Rebuilt in persistent WSL home and launched concurrently. `recovery-cohort/` includes all eight UARTs, builds, traces and manifests, and the supplemental inputs before and after removing the assertion. This reproduces the first fixture failures without replacing their lost original artifacts.
3. Windows-sealed serial cohort. Six individual launches in the table above, followed by one separate shipped-pair memory observation. The per-build manifests and UARTs identify the precise ELF identities. Rebuild paths can change artifact digests. Use the included ELFs for exact replay.

최초 임시 디렉터리 자료는 유실됐고 원인은 독립적으로 입증되지 않았다. 초기 수동 실행의 ROM 오류와 보조 예제 두 개의 주소 단정 실패(112)도 기록했다. 동시 실행한 2차 복구 기록은 별도 미채점 자료이고, 최종 채점은 Windows 디스크에 보관한 순차 실행만 사용했다.

## Reproduction / 재현

Extract the archive, then run `python3 check-apps.py` from any directory (Python 3.9+). It verifies file SHA-256, ELF/build identities against each manifest, complete UART sequences, timer exception routes, and physical descriptor/data decoding. `apps-files.sha256` includes SHA-256 for all archived payload files other than itself. Each serial workload directory contains the original command, UART, exception log, kernel ELF, application ELF(s), and build metadata. The build script needs Rust/Cargo 1.95.0, targets aarch64-unknown-none and aarch64-unknown-none-softfloat, clang, LLD and QEMU AArch64. Check out the stated Git commit and run `EVIDENCE=/absolute/extracted-directory bash /absolute/extracted-directory/reproduce-apps.sh`. For exact ELF replay use that workload's recorded command with paths adjusted to the extracted directory and a bounded timeout. The original guest remained parked after exit; the capture timeout ended QEMU.

압축 해제 후 `python3 check-apps.py`로 파일 무결성, UART 순서, 타이머 경로와 실제 매핑 결과를 오프라인 검증할 수 있다. 지정 커밋의 깨끗한 체크아웃에서 위 스크립트를 실행하면 새 빌드로 재현한다. 동일 바이너리가 필요하면 각 폴더의 ELF와 기록된 QEMU 명령을 사용한다.
