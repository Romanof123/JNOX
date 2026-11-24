# AArch64 architecture base verification / AArch64 아키텍처 기반 검증

Baseline `fb7d9429e00a3a41797dac7ac85f5e5a68602915`. Candidate `1cc77953c2faf74dfacdfde644d72a1ee0037ac4`, tree `5fe9a83354bd47f74328de4965c4e0fe497a83f2`. Contract CHG074 `dff321618c3da0b5dca9897373773e04719c1a5f`.

Both revisions used the same checkout path, Windows x86-64 MSVC Rust 1.95.0 and a fresh Cargo output directory per invocation. The only installed Rust targets under the pinned toolchain were the native Windows host and `aarch64-unknown-none`. The launcher advertised exactly `aarch64`. Three launcher unit tests passed.

두 커밋을 같은 경로에서 Rust 1.95.0으로 빌드했다. 매번 새 출력 디렉터리를 사용했다. 호스트 외에 설치된 타깃은 `aarch64-unknown-none` 하나였고 목록 출력은 `aarch64` 한 줄이었다.

## ELF comparison / ELF 비교

| Board and application | Entry | Baseline ELF SHA-256 | Candidate ELF SHA-256 |
|---|---:|---|---|
| virt default | 0x40200000 | 60684dd95ab040b125c6e6447b3946314c38df61fdcb19cb180d6db328459861 | same |
| pi4 default | 0x80000 | 0c90947941ae87ccf2c7af717c44c059d58e2e10c474121aba5d155ce9b45453 | same |
| virt fixed-fifo | 0x40200000 | b461010b36b99383063f7c5d425e6d9cba930a872c5ea8a286edae84365002e1 | 92511c2360275ad03467751bfb9c2db4e847ab93318652c4b4fc3846a3106565 |
| pi4 fixed-rr | 0x80000 | d8e30897ac42624c2fbe7b41c4402f1ebd7e9f5f4f54b35eacb1a89c4ad23058 | 935deba647bef1f7b0588cbf1d4a8a7713bfbb1b28d8842a98da50d33fd6cc12 |

Each pair had identical ELF entry, all PT_LOAD header fields and every file-backed byte of those segments. The two scheduled ELFs differed outside those segments. Direct Cargo with implicit or explicit architecture and the launcher with implicit or explicit architecture all produced the candidate virt default ELF above.

네 조합 모두 진입점, 모든 PT_LOAD 헤더 필드와 적재 바이트가 일치했다. FIFO·RR의 전체 ELF 해시 차이는 적재 구간 밖에 있다. 기본 보드에서 Cargo와 런처 모두 아키텍처 생략·명시 결과가 동일했다.

## Replay / 재현

From the complete checkout root, use a fresh absolute `CARGO_TARGET_DIR` for each build and clear inherited `JNOX_*` selectors and `CARGO_BUILD_TARGET`. POSIX shell examples follow. Preserve the sibling `arch/` and `kernel/` directories. Run the launcher commands on the candidate.

```sh
(cd kernel && cargo build --locked --release -j1)
(cd kernel && JNOX_ARCH=aarch64 cargo build --locked --release -j1)
python3 tools/build_kernel.py --list-arches
python3 tools/build_kernel.py
python3 tools/build_kernel.py --arch aarch64
(cd kernel && JNOX_BOARD=pi4 cargo build --locked --release -j1)
(cd kernel && JNOX_BOARD=virt JNOX_PROFILE=fixed JNOX_APP=fixed-fifo cargo build --locked --release -j1)
(cd kernel && JNOX_BOARD=pi4 JNOX_PROFILE=fixed JNOX_APP=fixed-rr cargo build --locked --release -j1)
```

The CLI rejected `--arch riscv64` and a conflicting `CARGO_BUILD_TARGET=x86_64-pc-windows-msvc` (exit 2). Direct Cargo rejected `JNOX_ARCH=riscv64` and an explicit `--target x86_64-pc-windows-msvc` in the build script (exit 101). Both rejected targets were checked without installing additional toolchains or targets.

기존 선택 환경변수를 비우고 빌드별로 새 절대 출력 경로를 지정한다. 지원하지 않는 이름과 타깃 불일치는 런처에서 종료 코드 2, 직접 Cargo에서 101로 거부됐다.

## Guest UART / 게스트 UART

Set `VIRT_ELF` and `PI4_ELF` to the corresponding default release artifacts (under each output directory at `aarch64-unknown-none/release/jnox-kernel`). Use a fresh capture directory per invocation.

```sh
python3 tools/qemu_boot.py --profile boot --qemu /usr/bin/qemu-system-aarch64 --kernel "$VIRT_ELF" --runs 1 --capture-dir virt-capture
aarch64-linux-gnu-objcopy -O binary "$PI4_ELF" kernel8.img
python3 tools/qemu_boot.py --profile pi4 --qemu /mnt/d/codex/arm-speed-lab/build/system/qemu-system-aarch64 --kernel kernel8.img --runs 1 --capture-dir pi4-capture
```

One cold boot per board per revision passed, including the observer's post-transcript quiet window. Linux QEMU 6.2.0 virt ran with one Cortex-A72 vCPU and TCG. The pinned raspi4b executable reported 11.1.3 and ran the board's ordinary boot CPU path under TCG using its default machine topology. The Pi raw image SHA-256 was `f57612bd6522c72af9c4eeec435cc04f30b5a3814b73829072d14553415ac6a0` on both revisions.

| Guest | Exact UART lines in order |
|---|---|
| virt | `JNOX M1 BOOT`, `JNOX M1 READY` |
| raspi4b | `JNOX PI4 BOOT`, `JNOX PI4 READY` |

두 커밋을 보드별로 한 번씩 콜드 부팅해 표의 UART 두 줄과 이후 무출력 구간을 확인했다. virt는 vCPU 1개, raspi4b는 모델 기본 토폴로지에서 부트 CPU 경로를 사용했다. 이 검증의 실행 범위는 에뮬레이터의 기본 이미지다. FIFO·RR은 빌드 및 적재 이미지 비교까지 확인했다. Physical Pi hardware, SMP behavior and scheduled guest execution were outside this gate.
