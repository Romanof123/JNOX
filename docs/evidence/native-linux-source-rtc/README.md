# Original Linux PL031 driver running on JNOX / Linux PL031 원본의 JNOX 직접 실행

**EVD-325 · REQ-004/011 · DES-002 · IMP-325 · VER-004/011.** JNOX booted its Rust kernel and two native user services on an AArch64 QEMU virt machine. Its slot-0 service contains the unmodified Linux v6.18 `drivers/rtc/rtc-pl031.c` compiled as a separate C object. Linux driver callbacks read and set the QEMU clock, configured and handled its alarm, and removed and rebound the device. The machine booted a single JNOX kernel ELF. [Implementation #425](https://github.com/Romanof123/JNOX/pull/425) · [Original C file](https://github.com/torvalds/linux/blob/v6.18/drivers/rtc/rtc-pl031.c) · [Run data](result.json)

QEMU에서 Linux 커널이나 하이퍼바이저 없이 JNOX만 부팅했다. 원본 C 파일의 SHA-256과 Git blob ID를 확인한 후 별도 오브젝트로 컴파일했다. 원본 콜백은 JNOX가 제공하는 AMBA 등록, 고정 크기 할당, RTC, 레지스터 접근 및 인터럽트 API를 사용했다. 서비스 ELF와 원본 오브젝트는 새 디렉터리에서 바이트 단위로 동일하게 다시 빌드됐다.

| Observation / 관측 | Result / 실측 |
| --- | --- |
| Original source / 원본 | Linux v6.18, 12,342 bytes, SHA-256 `3178d83f57c426bccb1a96cdceb3298b34651cc04c58b53c07a5dfc69bc94f76`, SPDX GPL-2.0-or-later |
| Device / 장치 | Real QEMU PrimeCell ID registers yielded peripheral ID `0x141031` and cell ID `0xb105f00d`. Slot 0 used protected MMIO at `0x09010000`. |
| RTC / 시계 | Read 2024-01-01 UTC, wrote 2032-02-29 23:59:58 UTC, read back the written seconds, observed the alarm at 2032-03-01 00:00:00 UTC. |
| Interrupt / 인터럽트 | First boot captured the PL031 assertion and clear. The repeat also captured CPU0 accepting GIC INTID 34 between hardware assertion and the driver's status read and clear. |
| Lifecycle / 수명 주기 | Two binds and two removals per run. Zero resources left at final release. |
| Isolation / 권한 | An ungranted process received −EACCES for claim, register read, register write and IRQ retrieval. Its independent timer recorded eight wakes. Neither application had an EL0-readable device page. |

**Interrupt delivery.** EL1 latches and masks the hardware line. The native service polls its pending alarm at 10 ms intervals, consumes the kernel event and invokes the original handler registered by `request_irq`. Its clear operation rearms the line. In the repeat capture the original handler reported one alarm with flags `0xa0`. Reprogramming the already reached match during disable caused a separate immediate device assertion and a second GIC acknowledgement. Asynchronous userspace callback delivery remains a follow-up.

EL1이 실제 IRQ를 수락하고 서비스가 확인할 이벤트를 보관한다. 이번 서비스는 최대 10 ms 간격으로 확인한 뒤 등록된 원본 핸들러를 호출했다. 검증 대상은 ARM PL031 한 종류다. ST 변형, 절전 복귀, 네트워크 드라이버와 DMA, 실보드 시간 보장 및 제품 안전 승인은 별도 범위다. [Next driver and DMA work #426](https://github.com/Romanof123/JNOX/issues/426)

**Import controls.** A single changed source byte was rejected before creating the output directory. Linking an additional relocatable object with an unsupported dependency caused the symbol check to reject the candidate. Independent negative controls exercised changed alarm events, register data, physical RAM receipts, process grant, ELF, and original digest. Seven altered records were rejected on the first capture and eight on the repeat, including removal of the GIC acknowledgement. [Native grant and checks](../../../tools/verify_linux_source_rtc.py) · [Recorded import profile](../../../drivers/linux-compat/profiles/rtc-pl031.json)

## Build and verify / 빌드와 검증

Use the Linux checkout and native AArch64 toolchain, clang/LLD, GNU binutils, Python 3 with pyelftools, and qemu-system-aarch64. Supply the exact original Linux file above as `source_c`. Only named, checked-in import profiles are admitted. The importer writes a relocatable driver object, a static native service ELF and an input/command manifest. Drivers needing additional Linux facilities require an implementation of those facilities and their own profile.

```sh
set -eu
work=$(mktemp -d)
python3 -B tools/import_linux_driver.py --profile rtc-pl031 --source "$source_c" --out "$work/import"
sh sdk/native-aarch64/build.sh examples/linux-source-rtc/peer.c "$work/peer.elf"
CARGO_BUILD_JOBS=1 python3 -B tools/build_native_el0.py --board virt --linux-source-rtc --policy fifo \
    --priority 8 --peer-priority 8 --elf "$work/import/service.elf" --peer-elf "$work/peer.elf" --out "$work/native"
python3 -B tools/verify_linux_source_rtc.py --package "$work/native" --driver-manifest "$work/import/import.json" --out "$work/capture"
python3 -B tools/verify_linux_source_rtc.py --replay --require-gic --package "$work/native" --driver-manifest "$work/import/import.json" --out "$work/capture"
```

**Originals / 원본.** The local packet `native-linux-source-rtc.tar.xz` is 932,552 bytes, SHA-256 `69dc2057eff26870c368f2281a4e4a47336aa06fb8f11bc95f632f10c50b9723`. It includes the upstream C file and license information, importer inputs, object and service ELF, both captures, mutation controls and build records. From a fresh extraction `sh replay.sh` checked file digests, replayed both captures, ran the evidence controls and rebuilt identical driver and service binaries. [Trace links](../../../trace/links.csv) · [Acceptance issue #424](https://github.com/Romanof123/JNOX/issues/424)
