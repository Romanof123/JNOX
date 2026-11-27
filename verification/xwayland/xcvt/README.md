# libxcvt for the Xwayland virtual output

Xwayland 24.1.6 calls libxcvt_gen_mode_info in hw/xwayland/xwayland-cvt.c, converts its kHz clock to Hz, and passes the resulting timing to RRModeGet. The probe calls that upstream library interface with nominal 1024×768 at 60 Hz. Its expected normal-blanking progressive mode has a 63.5 MHz clock, horizontal sync 1072–1176 of 1328 pixels, vertical sync 771–775 of 798 lines, negative HSync and positive VSync. The resulting refresh is about 59.92 Hz. This is a virtual mode calculation. Physical connector limits, scanout resolution, monitor acceptance, and measured frame timing require separate display evidence.

Download the [X.Org libxcvt 0.1.3 source archive](https://xorg.freedesktop.org/archive/individual/lib/libxcvt-0.1.3.tar.xz). The script checks the archive and MIT COPYING digests and installs its unmodified license. The upstream Meson target explicitly calls shared_library; the script archives that target’s single original source translation unit for static JNOX linking. Host Python, LLVM archive tools and pkg-config process only the pinned target inputs. The native graphics compiler wrapper supplies AArch64 Picolibc and startup.

Run with JNOX_GRAPHICS_RUNTIME set to the verified target runtime, then invoke sh build.sh ARCHIVE NEW_WORK NEW_TARGET_PREFIX. Package NEW_WORK/probe.elf with the native FIFO packager. Require a native exit of zero, the packaged ELF digest, and an executed libxcvt_gen_mode_info entry in the QEMU trace. Probe exit codes 1–6 distinguish allocation, dimensions, clock, horizontal timing, vertical timing, and polarity.

한국어: 원본 libxcvt의 CVT 계산 결과를 고정된 모드라인과 비교한다. 물리 디스플레이 출력 및 Xwayland의 RandR 등록 검증은 별도 단계이다.
