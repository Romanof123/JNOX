# Boot-provisioned UTC for gettimeofday / 부팅 기준 UTC

CHG-340 · REQ-005/008/014/018/024 · DES-003/009/013 · VER-005/008/014/018/024. Xwayland's time and logging paths read civil time through Picolibc. The native AArch64 virt FIFO/RR profile can represent an explicitly provisioned POSIX epoch, including dates beyond 2038. [Native link #471](https://github.com/Romanof123/JNOX/issues/471) · [System time plan](../../../docs/system-time-plan.en.md)

The package builder accepts `--boot-utc-seconds SECONDS`, a signed 64-bit count of POSIX seconds at the kernel's boot monotonic origin. Only the image producer can provision that value. The package manifest records the source, decimal epoch, reference and unknown uncertainty. A package built without this option records an unset source. The builder clears inherited JNOX configuration before assembling the image. The ordinary guest has read access and no setter.

The kernel samples the same counters used for the monotonic clock. It adds the elapsed whole seconds to the seed and truncates the remaining fraction to microseconds. It checks the signed result range before copying the two fields to EL0. The monotonic origin, counter rate, sleeps and deadline calculations retain their existing semantics. A caller replaying the image on a later day observes the same initial epoch. No current-date accuracy, RTC synchronization, drift or persistence has been established.

| Native SVC 207, x0 = timeval destination, x1 = timezone, x2 = 0 | Result |
| --- | --- |
| Valid seed, writable destination, null timezone | 0 and two signed LP64 fields, seconds and microseconds (0–999999) |
| No seed, non-null destination, null timezone | `ENODATA`, output preserved |
| Non-null legacy timezone | `ENOTSUP`, no writes |
| Null destination or inaccessible seeded destination | `EFAULT` |
| Epoch addition or elapsed counter outside supported range | `EOVERFLOW`, output preserved |

The existing Linux-source PL031 driver owns its RTC MMIO and IRQ only in a separately packaged driver profile. This UTC source requests no additional RTC rights. A deployment that needs actual current UTC must qualify its time authority, battery-backed retention, uncertainty, synchronization and holdover before using wall time for such decisions. [PL031 acceptance #424](https://github.com/Romanof123/JNOX/issues/424)

## Guest acceptance

Use a fresh output directory per mode and the verified Picolibc runtime. Run the two invocations sequentially under the shared build/guest lock. From the source checkout, use the following commands once with `mode=--utc-unset` and once with `mode=--utc-seeded`.

```sh
mkdir "$out"
export CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$out/target"
sh sdk/native-aarch64/build-rust.sh services/socket/Cargo.toml "$out/broker.elf"
python3 examples/native-xwayland-shm/graphics_dup_gate.py "$runtime" "$out" \
  "$(command -v qemu-system-aarch64)" "$mode"
```

The seeded fixture uses 2208988800 seconds (2040-01-01 UTC). Each successful sample must fall between independently sampled monotonic bounds after subtracting the seed. Both fixtures take a second sample after a one-second monotonic wait. Unset reads must report `ENODATA` every time and preserve the prior output. PID 0 returning 87 indicates that all checks in the selected fixture passed. Cases 121–129 identify the failed check. Record each package's `utc_clock` manifest and caller exit, then repeat the actual Xwayland link. [Guest caller](guest.c) · [Link procedure](../server-link.en-ko.md)

## 한국어

`--boot-utc-seconds`로 단조 시계 기점에 해당하는 UTC 초를 패키지에 명시한다. 생략하면 UTC는 미설정 상태이며 `gettimeofday()`가 `ENODATA`를 반환한다. 커널은 같은 하드웨어 카운터의 경과 시간을 더하고 범위를 확인한 후 초와 마이크로초를 쓴다. 패키지 기록에는 시각의 출처와 기준, 불확도 미확정 상태를 남긴다.

같은 이미지를 다른 날 재부팅해도 초기 UTC는 같은 값으로 시작한다. 실제 현재 시각이나 물리 RTC의 정확도는 별도로 검증해야 한다. 기존 PL031 드라이버의 소유권을 유지한다. 게스트 시험에서는 UTC 미설정 패키지와 2040년 기준 패키지를 각각 부팅해 시각 조회와 단조 대기, 오류 시 출력 보존을 확인한다.
