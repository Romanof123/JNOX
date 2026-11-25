# Native task policy example / 네이티브 태스크 정책 예제

## Build / 빌드

From the repository root on Linux, with Clang, LLD, Cargo, Python 3 and the Rust target `aarch64-unknown-none-softfloat` installed, run the following commands. Linux 저장소 루트에서 같은 명령을 실행합니다.

```sh
sh examples/native-task-policy/build.sh /tmp/jnox-policy
python3 tools/task_policy.py /tmp/jnox-policy/policy.txt --out /tmp/jnox-policy/on
python3 tools/task_policy.py /tmp/jnox-policy/unmonitored.txt --out /tmp/jnox-policy/off
python3 tools/task_policy.py /tmp/jnox-policy/scale20.txt --out /tmp/jnox-policy/scale
(cd /tmp/jnox-policy && sha256sum -c apps.sha256)
```

Each output directory contains the compiled policy and `policy-manifest.json` with each app's ELF digest, task entry addresses and settings. Both watchdog configurations use the same five ELF files. Only the final field of the `heavy burn` line changes. Keep the linked ELF symbol tables for entry resolution. 각 출력 디렉터리의 매니페스트에서 ELF 해시, 진입 주소, 정책을 확인할 수 있습니다. 감시 여부를 바꿀 때 앱을 다시 빌드할 필요가 없습니다.

## Policy / 정책

```text
version 1
app APP ELF_PATH
task APP TASK ENTRY PERIOD_MS PRIORITY DEADLINE_MS WATCHDOG_MS|off
```

Paths are relative to the policy file. Larger priorities run first. The deadline records late work. A numeric watchdog stops the offending task on expiry. `off` allows that task to continue. ELF 경로는 정책 파일 기준입니다. 우선순위 값이 클수록 먼저 실행됩니다. 마감 초과는 기록되고, 감시 시간이 지나면 해당 태스크가 중지됩니다. `off`는 감시를 끕니다.

| App | Task | Expected guest observation / 예상 관측 |
| --- | --- | --- |
| sensor (C) | sample, observe | Producer exits 21 after 20 increments. Observer exits 22 after eight distinct readings, or reports fault 9 on a zero or unchanged sequence. 생산자는 20회 갱신 후 상태 21로, 관측자는 새 순번을 8회 확인한 후 상태 22로 종료합니다. |
| heavy (C) | burn | Performs 100 million finite checksum updates without a service call. Watchdog should stop it. With monitoring off it exits 23. 감시를 끄면 계산을 마치고 상태 23으로 종료합니다. |
| late (C) | batch | Performs 40 million checksum updates and exits 24. A missed deadline should be recorded on the reference TCG run. 감시 없이 마감을 넘긴 작업이 상태 24로 종료합니다. |
| fault (C) | report | Reports fault code 37 immediately. 결함 코드 37을 직접 보고합니다. |
| heartbeat (Rust) | beat | Increments its private counter once per release and exits 25 when it reaches eight. 다른 앱이 중지된 후에도 작업을 이어가며 자체 카운터가 8에 도달하면 상태 25로 종료합니다. |

Loop durations depend on the host and emulator. These rows describe expected observations. Check the guest trace for actual dispatches, misses, stops and subsequent healthy releases. `scale20.txt` declares twenty separate app instances of the Rust heartbeat ELF. 각 반복문의 소요 시간은 호스트와 에뮬레이터에 따라 달라집니다. 실제 결과는 게스트 추적으로 확인합니다. 규모 예제는 같은 Rust ELF를 독립된 앱 20개로 선언합니다.

## Task ABI / 태스크 ABI

For the scale fixture, check that all twenty instances exit 25, each with its own counter at eight, followed by `JNOX POLICY DONE`. The scheduler also supports applications that continue completing periodic jobs indefinitely. 규모 예제에서는 20개 인스턴스 각각의 최종 카운터 8, 종료 상태 25 및 마지막 완료 메시지를 확인합니다. 주기 작업을 계속 수행하는 앱도 지원합니다.

The current interface uses native AArch64 v1 `svc #0x4a01`. Use `<jnox/task_policy.h>` in C and `jnox_native::task_policy` in Rust. All entries use the ordinary C calling convention and have no arguments. 현재 구현에서는 일반 C 호출 규약의 인자 없는 함수를 태스크 진입점으로 지정합니다.

| x8 | C / Rust | x0 |
| --- | --- | --- |
| 80 | `jnox_task_complete()` / `complete()` | Completes this job and returns zero on next release. 다음 주기 재개 시 0을 반환합니다. |
| 81 | `jnox_task_fault(code)` / `fault(code)` | Nonzero u32 fault code. Stops the caller on success. 성공 시 호출 태스크를 중지합니다. |
| 82 | `jnox_task_exit(status)` / `exit(status)` | Signed exit status. Stops the caller on success. 지정한 상태로 호출 태스크를 종료합니다. |

Failures return the signed native error in x0. The Rust fault wrapper takes `NonZeroU32`. 오류 시 x0에 부호 있는 오류 값이 반환됩니다.
