# Monotonic clock resolution / 단조 시계 해상도

[#471](https://github.com/Romanof123/JNOX/issues/471) · CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. Xwayland's `GetTimeInMillis` checks the resolution of a clock before selecting it. The supported public clock in the native fixed-priority graphics profile is `CLOCK_MONOTONIC`.

The native kernel reads `CNTFRQ_EL0` once during boot and rejects zero before entering user mode. Operation 200 on SVC `0x4a01` takes the native clock ID in x0 and an optional timespec destination in x1. It returns the duration of one hardware counter tick, rounded up to the next representable whole nanosecond. The kernel uses its existing checked 16-byte timespec copy. Native SDK and Picolibc callers receive 0 on success or -1 and errno on failure. A null destination succeeds for the supported clock. An unsupported ID returns `EINVAL` even with a null destination. An unwritable non-null destination returns `EFAULT`.

The resolution describes the time source. Timer wake-up latency, periodic scheduling and a wall-clock epoch each have separate acceptance requirements. The existing get-time service (6) and absolute-sleep service (7) retain their ABI.

| AArch64 virt guest observation required | Expected result |
| --- | --- |
| `clock_getres(CLOCK_MONOTONIC, &res)` on the 62.5 MHz QEMU profile | `{0, 16}` |
| Supported clock, null result | 0 |
| Unsupported clock, writable result or null | `-1/EINVAL`; the supplied result retains its old value |
| Supported clock, inaccessible result | `-1/EFAULT`; caller continues executing |

From the checkout, set `runtime` to the verified Picolibc graphics runtime and `out` to a fresh absolute directory. Run the following under the shared serialized build/guest lock.

```sh
mkdir "$out"
export CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$out/target"
sh sdk/native-aarch64/build-rust.sh services/socket/Cargo.toml "$out/broker.elf"
python3 examples/native-xwayland-shm/graphics_dup_gate.py "$runtime" "$out" \
  "$(command -v qemu-system-aarch64)" --clockres
```

The Picolibc caller returns 76 only after all assertions pass. Results 111–115 identify the failing assertion in [the caller](../../sdk/native-aarch64/graphics/os/clockres_caller.c). Verify that the native Xwayland executable's subsequent link no longer reports `clock_getres` under [the server acceptance procedure](server-link.en-ko.md).

고정 우선순위 그래픽 프로필에서 호출자에게 제공하는 단조 시계 해상도는 부팅 시 읽은 카운터 주파수로 계산한다. 한 틱을 나노초 단위로 올림해 반환한다. 커널은 쓰기 가능한 사용자 영역에만 결과를 기록한다. 출력 주소가 비어 있어도 지원하는 시계인지 확인할 수 있고, 지원하지 않는 시계는 `EINVAL`로 보고한다. 접근할 수 없는 출력 주소는 `EFAULT`로 보고한다.

이때 62.5 MHz QEMU에서 기대하는 해상도는 16 ns다. 위 직렬 실행의 앱 종료값 76과 전체 서버의 후속 링크 결과를 각각 확인한다. 스케줄링 지연, 실제 보드 및 기능안전 승인은 별도 검증 대상이다.
