# Native graphics C runtime

This opt-in build targets JNOX AArch64 EL0 with Clang and LLD. It compiles the original Picolibc 1.8.10 and Pixman 0.44.0 sources and keeps their license files under target/share/licenses/jnox-graphics. The build emits a target prefix, Meson cross file for graphics libraries, link map and provenance manifest. The existing SDK startup, linker script and Rust target remain separate.

Picolibc supplies its real nano allocator, memory and string operations, libm and tinystdio. Its sbrk uses an aligned 512 KiB process-private arena in the writable PT_LOAD. Startup invokes Picolibc's init array, which runs Pixman's constructor; normal termination invokes Picolibc's fini array. Explicit per-slot console grants enable bounded native SVC 4 writes for stdio and diagnostics. An assertion terminates the process with status 134. The image fits three distinct PT_LOAD segments in the existing 0x00400000–0x00800000 user window. The build rejects dynamic loading and TLS segments. This runtime is single-threaded, with one graphics context per process and no Linux userspace or semihost transport. The current integer-only formatted-I/O selection is independent of hardware double precision arithmetic.

Use a fresh output directory and the cached authenticated source inputs. The Picolibc orig checksum is d1fc2a20f217472f8d9da084a519a25678e8d91852a41c7f3f1840d3508f7163, as recorded in Keith Packard's signed Debian 1.8.10-2 DSC. The build checks the complete DSC hash, the VALIDSIG fingerprint C383B778255613DFDB409D91DB221A6900000011 and both source archive checksums. In the recorded environment the keyring came from debian-keyring_2025.07.26_all.deb (SHA256 b201fc5165cb91c0a8712c9a0fa8dded15729cb25cbe9d5e743986ec9b1256fb). The same Debian developer keyring can be supplied from an authenticated package installation.

```sh
python3 sdk/native-aarch64/graphics/build.py --inputs "$inputs" --keyring "$keyring" --out "$runtime"
python3 tools/build_native_el0.py --board virt --policy rr --quantum-ticks 625000 --console-write 0 --console-write 1 --elf "$runtime/probe.elf" --arg p --priority 4 --peer-elf "$runtime/probe.elf" --peer-arg q --peer-priority 4 --out "$normal"
python3 tools/build_native_el0.py --board virt --policy fifo --console-write 0 --elf "$runtime/probe.elf" --arg a --priority 4 --out "$assertion"
```

Capture the native virt boots with the supplied QEMU harness. It records UART and a symbol-filtered translated-block trace for the exact packaged ELF. The normal probe allocates and frees more than its heap capacity over time, checks ENOMEM, executes sqrt/floor and composites four premultiplied pixels using pixman_image_composite32. Two equal-priority processes keep different patterns in q0/q31, FPCR and FPSR across 32 real scheduler yields each. On this platform arch/aarch64/start.S sets CPACR_EL1.FPEN and native_el0_vectors.S saves/restores all Q registers and both controls. Verify the real guest workload before using that source path as evidence of working FP context switching. Record the normal private jnox_pixman_report symbol, linked map and each UART transcript alongside the native kernel ELFs.

```sh
python3 examples/native-pixman/capture.py "$normal/kernel.elf" "$normal_uart" 2
python3 examples/native-pixman/check_uart.py --trace "$normal_uart"
python3 examples/native-pixman/capture.py "$assertion/kernel.elf" "$assertion_uart" 1
python3 examples/native-pixman/check_uart.py --trace --assertion "$assertion_uart"

sh sdk/native-aarch64/build.sh examples/native-pixman/console_grant.c "$grant_elf"
python3 tools/build_native_el0.py --board virt --policy fifo --console-write 0 --elf "$grant_elf" --arg g --priority 4 --peer-elf "$grant_elf" --peer-arg d --peer-priority 4 --out "$grant_pkg"
python3 examples/native-pixman/capture.py --console-grant "$grant_pkg/kernel.elf" "$grant_uart" 2
python3 examples/native-pixman/check_uart.py --console-grant "$grant_uart"
```

The external oracle computes Porter-Duff OVER using integer arithmetic and checks both roles' output pixels, callback ordering and exits. It also requires both FP workloads to have begun before either finished. The assertion case checks the real assertion expression and abort diagnostic. The grant control runs the same ordinary SDK binary in two slots. The granted slot writes to fd 2 and checks EBADF and EFAULT. The ungranted slot checks EACCES for both output FDs. The Xwayland server still needs its remaining POSIX, event, protocol and device integration.

## 한국어

Picolibc와 Pixman 원본을 JNOX용으로 정적 교차 빌드한다. 전용 시작 코드에서 실제 생성자와 종료자를 실행하고 프로세스마다 512 KiB 힙을 할당한다. 표준 입출력·진단·종료는 JNOX 네이티브 ABI로 연결한다. 기존 C 및 Rust SDK의 기본 빌드는 유지한다. 대상은 단일 스레드이며 동적 로더와 TLS는 지원 범위에서 제외한다.

일반 부트에서는 두 프로세스가 32회씩 양보하면서 서로 다른 FP/SIMD 상태를 유지하는지 확인한다. 실제 Pixman 합성 결과는 독립적인 정수 계산식과 비교한다. 별도 부트에서 assert 진단과 종료 코드 134를 확인한다. 이 시험은 그래픽 라이브러리의 실행을 대상으로 하며 Xwayland 서버 부팅은 별도로 검증한다.

## Integration provenance / 통합 출처

The graphics branch starts at 93e41a9531ac1985675cfe9a9ce631b9f94e19cf. Its three QEMU boots used 6764dfed9f0556e47df3a1f6069b2fa5c95cfcbb, and 88afe7d94ddaec3fbdf54df2dda7e7c19f18249d records their original artifacts. The socket branch tested sealed transfers and ordinary poll at 5cc4884a292fb45844b53e80d4db0766def4ebfe and observed poll/EOF at f9db3c1be87db520aaf4357e733105694ecad90c. Its publication tree is ae67f22b45a6662c2a73a96f655a800dff6be401. These measured revisions identify separate guest packages. A build and boot of this combined tree remain pending.

그래픽 게스트 3회의 측정 소스는 6764dfed이고, 소켓·봉인 파일은 5cc4884a, 추가 poll/EOF는 f9db3c1b에서 측정했다. 두 브랜치를 결합한 트리의 빌드·부팅 검증은 별도로 남아 있다.
