# Kernel architecture selection

[한국어](README.ko.md) · [System architecture](../docs/architecture.en.md)

CHG-074 defines the build interface for the existing AArch64 kernel. The root selector and source layout are specified here for implementation. Build and boot verification for this change is pending.

From the repository root, list registered backends or build the kernel for AArch64:

```sh
python tools/build_kernel.py --list-arches
python tools/build_kernel.py --arch aarch64
```

Listing must print only `aarch64` and exit without starting Cargo. When --arch is omitted, use JNOX_ARCH if set and otherwise default to aarch64. A conflicting CARGO_BUILD_TARGET must fail before Cargo starts. The selected build uses the existing `aarch64-unknown-none` target and Cortex-A72 compiler settings. Direct Cargo invocation from `kernel/` keeps its existing default. Both entry points preserve the board, profile and application choices supplied through `JNOX_BOARD`, `JNOX_PROFILE` and `JNOX_APP`. A board selects its linker layout and devices independently of the architecture. [Cargo configuration](../kernel/.cargo/config.toml)

Each listed backend must have one canonical descriptor identifying its name, Rust target and implemented source. The sole initial backend under `arch/aarch64/` supplies real startup assembly or early CPU hooks linked into the kernel. Listing and selection consume the same registration. A directory name alone does not register a backend. Reject an unknown architecture or a conflicting explicit/environment target with a nonzero exit and a diagnostic identifying the supported name or expected triple before launching Cargo. Validate the effective Cargo target on the direct build path before linking.

For VER-008 compare the prior stock image, direct Cargo build and selected build with identical toolchain, board and workload inputs on QEMU virt and the Pi 4 model. Record actual target, linker entry, loaded ranges/bytes, raw Pi image and existing startup observations. Explain any differences against the baseline. Check listing and unsupported-name/target rejection independently, including that rejected wrapper inputs never launch Cargo. The qualification boundary is the exercised model configuration. [REQ-008](../docs/requirements.en.md) · [DES-004](../docs/architecture.en.md) · [VER-008](../docs/verification-plan.en.md)
