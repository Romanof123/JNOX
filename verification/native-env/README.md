# Native startup environment guest acceptance

The equal-priority yield fixture checks per-slot environment retention across a real process switch in both runtimes. 동일 우선순위의 두 프로세스가 실제로 전환된 뒤에도 각 슬롯의 환경이 유지되는지 검사한다. [Reproduction and measured results / 재현과 관측값](../../docs/evidence/native-environment-462.en-ko.md)

Run from a full checkout containing the packager, kernel and both SDK startup changes for #462. The selected Picolibc runtime directory supplies its manifest, headers and target libc archive. The runner recompiles both entry objects from the current checkout. Host tools are the ordinary native packaging prerequisites plus Clang/LLD, llvm-nm and qemu-system-aarch64.

```sh
python3 verification/native-env/run.py /absolute/path/to/new-results \
  --runtime /absolute/path/to/jnox-graphics-runtime --empty
```

Each package runs two copies of one ordinary AArch64 ELF. Slot 0 has role A, slot 1 has role B. Both receive a private argv vector and an ordered envp. WAYLAND_DISPLAY and XDG_RUNTIME_DIR differ by slot. EMPTY has a zero-length value. OPTION contains an additional equals sign and, in slot 1, a space. The packager arguments interleave entries for both slots. The test derives a checksum from the actual NUL-terminated entries passed to main and prints one bounded console receipt per role.

The Picolibc pair uses the same C source plus a constructor. It checks the selected libc’s getenv and environ, missing names and key prefixes before main. Main verifies the constructor’s private state, the original envp pointer, its final NULL and getenv again. QEMU records execution at main, the constructor and the linked getenv. The default command boots ordinary and Picolibc populated pairs. `--empty` also runs fully empty and mixed pairs for each ELF. Expected exits by PID are 73/74 (populated), 83/84 (fully empty), and 83/74 (mixed).

Each package directory contains its kernel and user ELFs, manifest, original UART, QEMU trace, symbol filter and independent checksum/exit comparison in receipt.json. A failure preserves the output produced before it.

동일한 ELF를 두 슬롯에 배치하여 다른 환경 변수의 순서·바이트·NULL 종결과 argv 뒤의 envp 위치를 확인한다. 선택한 Picolibc에서는 main 이전 생성자와 main 모두에서 getenv와 environ을 검사한다. `--empty`는 빈 환경 두 개와 한 슬롯만 채운 회귀 시험을 추가한다. 결과에는 원본 UART, 실제 실행 PC, 슬롯별 종료 코드, 환경 바이트로 계산한 체크섬이 남는다.
