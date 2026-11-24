# Build the kernel for AArch64

[한국어](README.ko.md) · [System architecture](../docs/architecture.en.md)

AArch64 is the only implemented backend. From the repository root, use Python 3 to list the available architectures or build the release kernel with the pinned Rust toolchain.

```sh
python tools/build_kernel.py --list-arches
python tools/build_kernel.py --arch aarch64
```

The first command prints `aarch64` without launching Cargo. The second builds for `aarch64-unknown-none`. Selection follows `--arch`, then `JNOX_ARCH`, then the default `aarch64`. An unknown name or a conflicting `CARGO_BUILD_TARGET` exits with an error before Cargo starts.

Use `JNOX_BOARD` to select the machine, `JNOX_PROFILE` for the runtime profile and `JNOX_APP` for the workload. The launcher passes these settings to Cargo. For example, in a POSIX shell:

```sh
JNOX_BOARD=pi4 JNOX_PROFILE=fixed JNOX_APP=fixed-fifo python tools/build_kernel.py --arch aarch64
```

To build directly with Cargo using the default architecture, run the following from the repository root. The same board, profile and application settings apply.

```sh
cd kernel
cargo build --locked --release -j1
```

The [target descriptor](aarch64/target) contains the Rust target triple. The [Rust module](aarch64/mod.rs) binds the byte-preserved [startup assembly](aarch64/start.S) to the kernel entry. Board-specific linker and device choices stay with the board configuration.

To add a backend, implement its startup code and register its target in `arch/<name>/target`. Wire the target configuration to that implementation, allow the matching Cargo target and provide a board linker layout. Verify entry, loaded bytes and startup on the intended machine before publishing the descriptor, since the launcher lists every descriptor it finds.

## Verification

Direct Cargo and the launcher produced the same default kernel. QEMU virt and the Pi 4 model both booted the AArch64 backend. Four build configurations matched the previous kernel in every loadable segment. The two scheduled images differed outside those segments. The launcher rejected unsupported names and conflicting targets. [CHG-074 independent verification](../docs/evidence/arch-base-first.md) · [REQ-008](../docs/requirements.en.md) · [DES-004](../docs/architecture.en.md) · [VER-008](../docs/verification-plan.en.md)
