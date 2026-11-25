# 네이티브 AArch64 C 응용 ABI v1

[English](native-c-abi.en.md) · CHG-086 · #21 및 #24 부트스트랩 설계안 · [POSIX 소스 프로파일](posix-profile.md) · [사용자 프로그램 아키텍처](architecture.md)

첫 응용 예제는 실행 인자로 파일 경로 하나를 받고 십진 센서 기록을 읽는다. 요약은 파일 서술자 1, 오류 메시지는 2에 쓰고 main의 반환값을 종료 상태로 사용한다. 같은 C 소스를 대상으로 ELF 생성, EL0 진입, 파일 서비스 동작과 응용 종료를 각각 검증한다.

## 실행 파일과 초기 스택

리틀엔디언 AArch64 LP64, AAPCS64 정수 호출 규약, 정적 ET_EXEC를 사용한다. Clang의 aarch64-none-elf 타깃과 LLD로 링크한다. 파일 바이트가 있는 RX 세그먼트의 첫 주소와 _start는 0x00400000이다. VA [0x00400000,0x00800000)에 필요한 R과 RW를 분리하여 최대 8개 LOAD를 허용한다. 파일 오프셋과 시작 주소를 4096바이트로 정렬하고 페이지 간 중복 및 쓰기·실행 동시 권한을 금지한다. BSS를 포함해 메모리에만 있는 꼬리 영역은 0으로 초기화한다. 런타임 재배치, 인터프리터, 동적 링크, TLS 세그먼트는 지원 범위 밖이다. SDK는 범용 레지스터만 사용한다.

신뢰된 실행 구성이 native v1을 선택한다. 읽기 전용 LOAD의 SHF_ALLOC SHT_NOTE 섹션 .note.jnox.abi를 확인한다. 노트는 리틀엔디언 namesz=5, descsz=4, type=1, 소유자 JNOX와 NUL, 0 패딩 3바이트, u32 버전 1로 구성된다. 별도 PT_NOTE는 필요 없다. 실행 권한은 출처 검증, 로더 검증 및 프로세스 권한 검사로 부여한다. 설정과 ABI가 다르면 진입 전에 거절한다.

로더는 이미지 밖에 쓰기 가능하고 실행 불가능한 스택을 준비한다. SP는 16바이트 경계이고 첫 u64는 0..INT_MAX 범위의 argc다. SP+8에 argc개의 NUL 종료 문자열 포인터를 놓고 마지막에 널 포인터를 붙인다. 실행기는 개수, 주소와 문자열 범위를 검사한다. envp와 auxv는 사용하지 않는다. 시작 코드는 x0, x1로 argc, argv를 main에 전달하고 int 결과를 부호 확장해 exit을 요청한다. 종료 후 복귀하면 예외를 발생시킨다. 기본 시작 구성은 주소 공간별 단일 스레드와 기존 errno 저장소를 사용한다. 두 스레드 옵션에서는 메인과 작업 스레드에 각자의 errno 셀을 할당한다. 비동기 신호 처리의 안전성은 별도 검증 대상이다.

## 서비스 규약

EL0가 SVC #0x4a01을 실행한다. x8은 서비스 번호, x0~x2는 인자다. EL1은 x0에 부호 있는 LP64 결과를 쓰고 나머지 범용 레지스터와 SP를 보존한다. 확장은 새 번호를 배정하며 기존 번호와 의미를 유지한다. 알 수 없는 번호에는 -ENOSYS(38)를 반환한다.

| 번호 | 호출 | 인자 | 성공 결과 |
|---|---|---|---|
| 1 | exit | 부호 있는 int 상태 | 프로세스 종료 |
| 2 | open | 경로 포인터, 플래그, 0 | 음이 아닌 int fd. O_RDONLY=0 지원 |
| 3 | read | fd, 쓰기 가능한 사용자 버퍼, size_t 길이 | 읽은 바이트 수, EOF이면 0 |
| 4 | write | fd, 읽기 가능한 사용자 버퍼, size_t 길이 | 기록한 바이트 수 |
| 5 | close | fd | 0 |

오류 범위는 -1..-4095다. C 래퍼는 오류에서 -1을 반환하고 양의 오류 번호를 errno에 기록한다. 성공하면 직전 errno를 유지한다. EINTR=4이면 응용이 재시도를 결정한다. 일부만 처리하면 실제 바이트 수를 반환한다. 지원하지 않는 open 플래그는 EINVAL=22로 실패한다. 처리량은 ssize_t 범위 안에 있어야 한다. EL1은 경로 검사 길이 및 전송량 제한, 버퍼 범위·방향 확인, 서술자 권한 검사, 종료 시 자원 회수를 구현해야 한다. 실행기가 표준 출력과 오류 출력을 연결해야 한다. SDK의 호출 발행 이후 파일과 장치에서 관측되는 동작은 커널 서비스와 실제 실행을 통해 검증한다.

## 검증

동일한 예제와 SDK를 포함한 체크아웃에서 다음 명령을 실행한다.

    sh sdk/native-aarch64/build.sh examples/portability/telemetry_monitor.c telemetry-native.elf

컴파일러와 링커 버전, 소스·ELF 해시, 프로그램 헤더, 섹션 권한, SVC 명령 및 외부 런타임 심벌 유무를 보존한다. 독립 로드 플래너로 생성된 바이트를 검증한다. 이후 게스트 검증에서 원래 argv 경로, 실제 파일 읽기, 부분 전송 및 오류, 표준 출력·오류 바이트, 종료 상태를 확인한다. Linux 바이너리 호환성, 추가 POSIX 인터페이스, 동시성과 TLS는 각각 독립적으로 검증한다.

## 네이티브 시계 앱 패키징 및 실행

서비스 6은 `clock_gettime(CLOCK_MONOTONIC, tp)`를 제공한다. `svc #0x4a01` 호출 시 x8에 6, x0에 시계 ID 1, x1에 쓰기 가능한 LP64 timespec 주소를 전달한다. 구조체에는 부호 있는 64비트 초와 나노초가 순서대로 들어간다. 성공하면 0을 반환하며 오류는 위의 네이티브 규약을 따른다.

Linux 저장소 루트에서 네이티브 빌드 도구, QEMU, AArch64 nm 및 Rust 타깃 `aarch64-unknown-none-softfloat`를 준비하고 새 출력 디렉터리를 사용한다.

    python3 tools/build_native_el0.py --source examples/portability/clock_checksum.c --out c-package
    sh sdk/native-aarch64/build-rust.sh examples/native-clock-rust/Cargo.toml clock-rust.elf
    python3 tools/build_native_el0.py --elf clock-rust.elf --out rust-package
    python3 tools/native_el0_capture.py c-run c-package/kernel.elf c-package/user.elf examples/portability/clock_checksum.c "$(git rev-parse HEAD)" --expect-exit 0
    python3 tools/native_el0_capture.py rust-run rust-package/kernel.elf rust-package/user.elf examples/native-clock-rust/src/main.rs "$(git rev-parse HEAD)" --expect-exit 0

매니페스트에 선택한 C 파일 또는 링크된 ELF의 경로와 SHA-256을 기록한다. 입력을 지정하면 일반 네이티브 프로세스 실행기가 argv[0]을 /bin/app으로 설정한다. 캡처는 Rust 소스를 포함해 전달받은 소스를 기존 이름 `probe.c`로 보관하며 UART 마지막 줄에서 예상 종료값을 확인한다. 각 캡처가 출력한 세션 해시를 아래 명령에 전달한다. 리플레이는 내장 ELF, 사용자 페이지 매핑, EL0 시계 호출 두 건의 성공, QEMU 예외 전환 및 종료값 0을 확인한다.

    python3 tools/native_el0_replay.py c-run "$C_SESSION_SHA256" --clock-app
    python3 tools/native_el0_replay.py rust-run "$RUST_SESSION_SHA256" --clock-app

기본 리플레이는 기존 시작 프로브의 종료값 37을 검사한다. 해당 대조군은 `python3 tools/build_native_el0.py --out startup-package`로 빌드한다.

네이티브 서비스 작업은 #286에서 추적하며 관련 이슈는 #12, #8, #21, #162이다. REQ-014는 개발·관측 API를 다루며 REQ-018은 시계 고장 등의 독립적인 감지와 대응을 요구한다. [요구사항](requirements.md)

## 단조 시계의 절대 기한까지 대기

기한은 같은 부팅 구간의 단조 시계에서 정한다. 주기 보고에서는 직전 기한에 주기를 더하면 복귀가 늦어져도 다음 기한은 원래 일정에 맞춰진다. 대상은 깨어 있는 단일 AArch64 QEMU virt CPU의 네이티브 프로세스 하나다.

C에서는 `clock_gettime(CLOCK_MONOTONIC, &target)`으로 읽은 시각에 주기를 더하고 나노초를 정규화한 뒤 `clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &target, 0)`을 호출한다. 반환값을 직접 확인한다. 성공하면 0, 실패하면 양의 오류 번호를 반환한다. `errno`와 호출자가 제공한 잔여 시간 구조체는 보존된다. [C 예제](../examples/portability/periodic_report.c)는 40 ms 간격으로 세 번 대기한다.

Rust에서는 `jnox_native::monotonic_time()?`으로 얻은 `core::time::Duration`에 `checked_add`로 주기를 더한 후 `jnox_native::sleep_until(target)?`을 호출한다. 이미 지난 기한이면 즉시 성공한다. [Rust 예제](../examples/native-periodic-rust/src/main.rs)는 매번 복귀 후 시계를 읽어 기한에 도달했는지 검사한다.

Linux 또는 WSL에서 네이티브 SDK 빌드·캡처 도구를 설치한 뒤 저장소 루트에서 두 예제를 패키징하고 실행한다.

~~~sh
run=$(mktemp -d)
rev=$(git rev-parse HEAD)
python3 tools/build_native_el0.py --source examples/portability/periodic_report.c --out "$run/c"
sh sdk/native-aarch64/build-rust.sh examples/native-periodic-rust/Cargo.toml "$run/report.elf"
python3 tools/build_native_el0.py --elf "$run/report.elf" --out "$run/rust"
python3 tools/native_el0_capture.py "$run/c-first" "$run/c/kernel.elf" "$run/c/user.elf" examples/portability/periodic_report.c "$rev" --expect-exit 0
python3 tools/native_el0_capture.py "$run/rust-first" "$run/rust/kernel.elf" "$run/rust/user.elf" examples/native-periodic-rust/src/main.rs "$rev" --expect-exit 0
~~~

## 협력형 네이티브 프로세스 두 개 실행

Rust 동료 앱을 별도로 빌드한 후 그 ELF를 주 C 앱 소스와 함께 패키저에 전달한다. 이미 링크한 주 앱을 사용하려면 `--source PATH.c` 대신 `--elf PATH.elf`를 지정한다. Linux 또는 WSL에서 저장소 루트를 기준으로 실행한다.

~~~sh
run=$(mktemp -d)
sh sdk/native-aarch64/build-rust.sh examples/native-cooperative-rust/Cargo.toml "$run/peer.elf"
python3 tools/build_native_el0.py --source examples/portability/native_cooperative_report.c --peer-elf "$run/peer.elf" --out "$run/package"
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel "$run/package/kernel.elf"
~~~

[주 C 앱](../examples/portability/native_cooperative_report.c)은 `<sched.h>`의 `sched_yield()`를 호출한다. [Rust 동료 앱](../examples/native-cooperative-rust/src/main.rs)은 안전한 `jnox_native::yield_now()`를 호출해 `Result<(), i32>`를 받는다. 양보하면 준비된 동료에게 CPU0을 넘기고, 준비된 동료가 없으면 즉시 성공한다. 절대 기한 대기는 호출자만 차단하므로 준비된 동료는 실행을 계속한다. 각 프로세스가 전용 스택과 쓰기 가능 데이터를 사용해 두 예제의 누계 변수는 같은 가상 주소 `0x402000`에서 서로 다른 값을 유지한다. UART에 각 PID와 종료값이 표시된다. 두 앱이 종료하면 게스트가 정지 대기에 들어간다. Ctrl+C로 QEMU를 종료한다.

## 독립 ELF 앱의 스케줄링

주 앱은 `--source` 또는 `--elf`로 지정하고 `--peer-elf`를 두 번까지 추가한다. 주 앱은 슬롯 0이고 동료는 입력 순서대로 슬롯 1, 2를 받는다. 정책 생략 시 협력형으로 동작한다. `--policy fifo` 또는 `--policy rr` 지정 시 주 앱에 `--priority` 하나, 동료마다 `--peer-priority` 하나가 필요하다. 우선순위는 0~255이며 큰 값이 먼저 실행된다. RR의 공통 퀀텀은 1~9223372036854775807 물리 카운터 틱이다. FIFO는 퀀텀을 받지 않는다.

Linux 또는 WSL에서 아래 명령으로 세 ELF를 각각 빌드하고 부팅한다. 패키지 매니페스트에는 슬롯 순서대로 입력 경로와 해시, ELF 해시, 우선순위가 기록된다. 마지막 앱이 종료되고 커널이 대기 상태에 들어간 뒤에도 시리얼 콘솔은 연결을 유지한다.

~~~sh
run=$(mktemp -d)
sh sdk/native-aarch64/build.sh examples/native-fixed/calendar_report.c "$run/high.elf"
sh sdk/native-aarch64/build.sh examples/native-fixed/prime_low.c "$run/a.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-fixed-rust/Cargo.toml "$run/b.elf"
python3 tools/build_native_el0.py --out "$run/rr" --elf "$run/high.elf" \
    --peer-elf "$run/a.elf" --peer-elf "$run/b.elf" --policy rr \
    --priority 7 --peer-priority 3 --peer-priority 3 --quantum-ticks 1250000
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 \
    -accel tcg -m 128M -nodefaults -display none -monitor none -no-reboot \
    -serial stdio -kernel "$run/rr/kernel.elf"
~~~

작은 예제에서는 3순위 계산 두 개가 여러 퀀텀에 걸쳐 실행되도록 1,250,000틱을 사용한다. 최초 고정 예제의 설정은 12,500,000틱이었다. 퀀텀은 패키지에서 물리 카운터 틱 단위로 지정한다. [네이티브 FIFO·RR 근거](evidence/evd-115.md)

## 네이티브 앱의 가중 CPU 서비스

주 ELF에 `--weight` 하나를 지정하고 동료 ELF마다 입력 순서에 맞춰 `--peer-weight` 하나를 지정한다. 1에서 4,294,967,295 사이의 가중치는 앱들이 계속 준비 상태일 때의 상대 CPU 서비스 비율을 지정한다. 같은 패키지의 앱은 물리 카운터 틱 단위의 공통 실행량을 사용한다. Linux 또는 WSL에서 저장소 루트를 기준으로 C CRC-32 계산과 Rust 비트 개수 히스토그램을 패키징한다.

~~~sh
run=$(mktemp -d)
sh sdk/native-aarch64/build.sh examples/native-weighted/checksum.c "$run/crc.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-weighted-rust/Cargo.toml "$run/hist.elf"
python3 tools/build_native_el0.py --out "$run/weighted" --elf "$run/crc.elf" \
    --peer-elf "$run/hist.elf" --policy weighted --weight 1 --peer-weight 2 --quantum-ticks 625000
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel "$run/weighted/kernel.elf"
~~~

[examples/native-weighted](../examples/native-weighted/README.md) · [EVD-116](evidence/evd-116.md)

## 네이티브 앱의 격자 기반 EDF

독립 빌드한 C·Rust ELF 1~3개를 `--policy edf`로 패키징한다. 주 앱의 튜플은 `--job`, 각 동료의 튜플은 해당 ELF 순서대로 `--peer-job`으로 지정한다. 튜플은 공통 기준 시점에서 100 ms 단위로 센 시작 가능 시각·절대 마감·예상 CPU 비용이다. 이 CPU0 유한 수용 프로필에서는 0 ≤ 시작 < 마감 ≤ 16이고 비용은 양수여야 한다. 수용 검사는 독립 작업의 선언 비용을 사용하며 실제 종료 시각은 따로 확인한다. 예를 들어 `1,4,1`은 100 ms에 시작 가능하고 마감은 400 ms, 예상 CPU 비용은 100 ms라는 뜻이다. Linux 또는 WSL에서 저장소 루트를 기준으로 실행한다.

~~~sh
run=$(mktemp -d)
sh sdk/native-aarch64/build.sh examples/native-edf/batch.c "$run/batch.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-edf-rust/Cargo.toml "$run/histogram.elf"
sh sdk/native-aarch64/build.sh examples/native-edf/urgent.c "$run/urgent.elf"
python3 tools/build_native_el0.py --out "$run/edf" --policy edf \
    --elf "$run/batch.elf" --job 0,8,2 \
    --peer-elf "$run/histogram.elf" --peer-job 0,12,8 \
    --peer-elf "$run/urgent.elf" --peer-job 1,4,1
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel "$run/edf/kernel.elf"
~~~

[examples/native-edf](../examples/native-edf/README.md) · [EVD-117](evidence/evd-117.md)

## 네이티브 주기 작업의 RM·DM 실행

독립 빌드한 앱 1~3개를 패키징한다. 각 튜플은 `초기위상_ns,주기_ns,상대마감_ns` 순서다. 마감은 양수이며 주기 이하여야 하고 양의 시간은 물리 타이머 틱으로 정확히 변환돼야 한다. 아래 C 앱은 주기 100 ms·마감 90 ms, Rust 앱은 주기 120 ms·마감 40 ms다. RM은 주기가 짧은 앱을, DM은 상대 마감이 짧은 앱을 먼저 실행한다. Linux 또는 WSL에서 저장소 루트를 기준으로 실행한다.

~~~sh
run=$(mktemp -d)
sh sdk/native-aarch64/build.sh examples/native-periodic-job/crc.c "$run/crc.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-periodic-job-rust/Cargo.toml "$run/hist.elf"
for policy in rm dm; do
    python3 tools/build_native_el0.py --out "$run/$policy" --policy "$policy" \
        --elf "$run/crc.elf" --periodic 0,100000000,90000000 \
        --peer-elf "$run/hist.elf" --peer-periodic 0,120000000,40000000
done
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel "$run/rm/kernel.elf"
~~~

DM 패키지는 커널 경로를 `$run/dm/kernel.elf`로 바꿔 부팅한다. 프로세스는 main에 한 번 진입한다. 배치 하나를 완료하면 C는 `<jnox/periodic.h>`의 `jnox_periodic_wait_next()`를 호출한다(성공 0, 실패 −1과 errno). Rust는 `jnox_native::wait_next_period()`를 호출해 `Result<(), i32>`를 받는다. 성공 시 같은 프로세스가 전용 데이터와 스택을 보존한 채 다음 세대를 시작한다. 종료는 현재 세대를 완료한다. [EVD-118](evidence/evd-118.md)

## 네이티브 앱의 CPU 예산 예약

Linux 또는 WSL에서 저장소 루트를 기준으로 [C CRC 작업과 Rust 히스토그램 작업](../examples/native-cbs/README.md)의 SDK 빌드 및 QEMU 실행 명령을 사용한다. 두 ELF를 패키징하는 명령은 다음과 같다.

~~~sh
python3 tools/build_native_el0.py --out isolated --policy cbs --elf crc.elf --reservation 1,4 --peer-elf histogram.elf --peer-reservation 1,4
~~~

ELF 순서에 맞춰 예산,주기 튜플을 하나씩 지정한다. 단위는 50 ms 슬롯 수다. 이 예제에서 각 앱은 네 슬롯(200 ms)마다 명목 실행권 하나를 받으며 사용하지 않은 용량은 유휴 상태로 남는다. 예산은 주기 이하인 양의 정수이고 각 예산/주기 분수의 합은 1 이하여야 한다. 주 앱 외에 동료 ELF를 두 개까지 지정할 수 있다. 각 앱은 실행권 사이에도 자기 블록 수와 계산 결과를 유지한다. 실제 CPU 서비스는 명목 실행권과 별도로 측정한다.

## 유한 네이티브 작업의 격자형 LLF

네이티브 EDF와 같은 100 ms 격자의 시작 가능 시각·절대 마감·예상 비용 튜플에 `--policy llf`를 지정한다. 커널은 격자 표본에서 부호 있는 여유 시간(마감 − 격자 번호 − 잔여 비용)이 가장 작은 작업을 선택한다. 새 작업이 들어오지 않아도 대기 작업이 선점할 수 있고 여유 시간이 같으면 실행 중인 작업을 유지한다. 예를 들어 체크섬 작업은 `--job 0,6,1`, 독립 빌드한 히스토그램은 `--peer-job 0,7,3`으로 지정한다. [실행 예제](../examples/native-llf/README.md)에 두 히스토그램 입력의 SDK 빌드·패키지·부팅 명령이 있다.

## 네이티브 고정 클래스 및 피드백 큐

고정 클래스는 `--policy mlq`로 지정한다. 주 앱에 `--class`, 각 동료에 ELF 순서대로 `--peer-class`를 지정하며 큰 클래스가 먼저 실행된다. `--quantum-ticks`는 같은 클래스의 순번 길이다. 예를 들어 경보 보고 앱을 클래스 7, CRC 앱을 클래스 1로 지정한다.

~~~sh
python3 tools/build_native_el0.py --out strict --policy mlq --quantum-ticks 625000 --elf report.elf --class 7 --peer-elf crc.elf --peer-class 1
~~~

피드백 큐는 같은 ELF 인자에 `--policy mlfq --allotment-ticks 625000,1250000,2500000 --boost-ticks 6250000`를 사용하고 클래스 인자를 생략한다. 시간 단위는 물리 카운터 틱이다. 기준 62.5 MHz 시계에서 세 단계의 할당량은 10·20·40 ms, 우선순위 복구 간격은 100 ms다. [실행 예제](../examples/native-multilevel/README.md)에서 C 보고·CRC·양보 앱 및 Rust 히스토그램 빌드와 패키지 부팅 명령을 확인할 수 있다.

## 네이티브 프로세스별 시작 인수

슬롯 0·1·2에 각각 `--arg`, `--peer-arg`, `--peer2-arg`를 반복해 UTF-8 문자열을 하나씩 추가한다. 첫 인수는 /bin/app이다. 공백은 따옴표로 묶고 빈 문자열은 명시적으로 전달한다. 대시로 시작하는 값은 `--arg=-west`처럼 지정한다. 슬롯별로 추가 문자열은 최대 15개이며 각 NUL 종료 문자를 포함해 총 1,024바이트까지 허용한다. 네이티브 main은 argc와 argv를 자기 스택에서 받는다. 추가 인수가 없는 슬롯은 SP 0x01010fb0의 기존 80바이트 레이아웃과 포인터·종료값·패딩을 보존한다. 시작 프로브도 기존 별도 80바이트 레이아웃을 유지한다.

~~~sh
sh sdk/native-aarch64/build.sh examples/native-argv/sensor.c sensor.elf
python3 tools/build_native_el0.py --out sensor-pair --elf sensor.elf \
    --arg sum --arg 16 --arg 32 --arg 'west rack' \
    --peer-elf sensor.elf --peer-arg minmax --peer-arg 64 --peer-arg 96 --peer-arg ''
~~~

[패키지 부팅과 Rust UTF-8 예제](../examples/native-argv/README.md)

## 네이티브 작업 스레드 생성 및 조인

저장소 루트에서 C 소스 하나 또는 링크된 Rust ELF 하나에 `--threads 2`를 지정한다. CPU0에서 프로세스 하나의 메인과 재사용 가능한 작업 스레드 하나가 협력형으로 실행된다. 전역 데이터와 argv 문자열은 프로세스 안에서 공유하며 스택·저장 레지스터·errno 셀은 스레드마다 따로 둔다.

~~~sh
python3 tools/build_native_el0.py --out output/native-threads/c --threads 2 --source examples/native-threads/sensor.c
~~~

메인은 `pthread_create(&handle, NULL, start, arg)`로 작업을 큐에 넣고 다음 스케줄링 지점까지 계속 실행한다. 작업 스레드는 메인만 조인할 수 있다. `pthread_join(handle, &result)`는 실행 중이면 기다리고 이미 종료됐으면 즉시 결과를 회수한다. 조인 뒤 슬롯을 재사용한다. 슬롯이 사용 중이면 EAGAIN, 회수된 핸들은 ESRCH다. 두 함수는 양의 오류 번호를 직접 반환하고 errno를 보존한다. 작업은 종료 뒤에도 유효한 공유 데이터 포인터를 반환할 수 있다. [C·Rust 빌드 및 부팅 예제](../examples/native-threads/README.md)

## 한 네이티브 프로세스 안의 라운드로빈

`--threads 2`만 지정하면 메인과 작업 스레드는 기본 협력형으로 실행된다. `--policy rr --quantum-ticks 625000`을 추가하면 준비된 동순위 스레드 둘이 CPU0에서 교대로 실행된다. 기준 62.5 MHz 시계의 이 퀀텀은 10 ms다. 두 스레드는 앱 주소 공간을 공유하고 저장 프레임과 스택은 각각 유지한다. [같은 스트림 ELF를 두 정책으로 빌드·부팅하기](../examples/native-thread-rr/README.md)

~~~sh
python3 tools/build_native_el0.py --out output/thread-rr/rr --source examples/native-thread-rr/stream.c --threads 2 --policy rr --quantum-ticks 625000
~~~

## 프로세스 전용 뮤텍스

협력형 및 RR 두 스레드 패키지에서 고정 주소의 프로세스 전용 `pthread_mutex_t`를 `pthread_mutex_init(&m, NULL)`로 초기화한다. SDK는 `pthread_mutex_lock`, `pthread_mutex_trylock`, `pthread_mutex_unlock`, `pthread_mutex_destroy`를 제공한다. 한 프로세스에서 뮤텍스 최대 4개를 동시에 사용할 수 있다. 사용 중인 잠금에 trylock을 호출하면 errno를 보존하며 양의 EBUSY를 직접 반환한다. 차단된 lock은 소유자가 잠금을 양도한 뒤 복귀한다. 모든 사용이 끝난 뒤 잠금이 풀린 상태에서 파괴한다. [C·Rust 공유 센서 빌드 및 부팅](../examples/native-thread-mutex/README.md)

## 두 마감 작업과 반복 서비스의 시간표

일반 작업 튜플 2개에 `--deadline-service-elf service.elf`, 오름차순 시간표(예: `--deadline-service-cells 1,3`), `--deadline-switch-cap 6`을 지정한다. 시간표는 100 ms 슬롯 16개로 구성된다. 서비스는 한 번 시작해 argv[1]에서 시간표를 읽는다. 마지막을 제외한 각 계산 뒤 C의 `<jnox/deadline.h>`에 선언된 `jnox_deadline_service_wait_next()` 또는 Rust의 `jnox_native::deadline_service_wait_next()`를 호출하면 다음 활성화 순번을 받는다. 마지막 계산 후 종료하며 예약 슬롯의 남은 시간은 유휴로 둔다. [빌드·패키징·부팅 예제](../examples/native-deadline-service/README.md)

## Pi 4 모델에서 두 네이티브 앱 실행

저장소 루트에서 C 및 Rust 소수 계산 앱을 빌드한다. 같은 두 ELF를 우선순위 2로 패키징해 CPU0에서 FIFO 또는 RR로 실행한다. 퀀텀 단위는 타이머 틱이다.

~~~sh
sh sdk/native-aarch64/build.sh examples/native-fixed/prime_low.c low.elf
sh sdk/native-aarch64/build-rust.sh examples/native-fixed-rust/Cargo.toml high.elf
python3 tools/build_native_el0.py --out pi-fifo --board pi4 --policy fifo --elf low.elf --priority 2 --peer-elf high.elf --peer-priority 2
python3 tools/build_native_el0.py --out pi-rr --board pi4 --policy rr --quantum-ticks 1250000 --elf low.elf --priority 2 --peer-elf high.elf --peer-priority 2
qemu-system-aarch64 -M raspi4b -accel tcg -display none -serial stdio -kernel pi-rr/kernel8.img
~~~

FIFO로 실행하려면 마지막 이미지 경로를 `pi-fifo/kernel8.img`로 바꾼다. 직접 적재는 EEPROM·SD·VideoCore 펌웨어를 거치지 않는다. 아래 명령으로 FAT32 카드에 복사할 고정 펌웨어를 준비한다. 이미 내려받은 펌웨어는 `--firmware-dir pinned-boot`로 지정한다. 실물 부팅과 보드 시간 측정은 별도 검증 대상이다. [카드 준비와 확인](boards/pi4-p1.md)

~~~sh
python3 tools/package_pi4.py --kernel pi-rr/kernel8.img --output pi-rr-boot
~~~

## 두 앱에 반복 CPU 창 지정

단일 CPU AArch64 virt에서 주 ELF는 a, 피어 ELF는 b다. 아래 시간표는 a 100 ms, b 50 ms, 커널 유휴 50 ms를 반복한다. 두 앱을 모두 포함하는 2~16개의 창을 지정할 수 있다. 지정한 앱이 차단되거나 종료되면 그 창의 남은 시간은 유휴 상태다.

~~~sh
python3 tools/build_native_el0.py --out table-p --policy partition --partition-windows a:100,b:50,idle:50 --elf energy.elf --peer-elf density.elf
~~~

[에너지 합계와 비트 밀도 히스토그램 빌드 및 QEMU 실행](../examples/native-cyclic/README.md)

## 비주기 서비스와 백그라운드 앱 실행

단일 CPU virt에서 주 ELF는 서비스이고 피어 ELF는 백그라운드 앱이다. 아래 예는 50 ms 슬롯 5개당 2개를 서비스 예산으로 정한다. 서비스에 슬롯을 배정할 때 예산 한 단위를 사용한다. 서비스가 차단되면 준비된 백그라운드가 남은 시간을 쓸 수 있다. 산발 서버의 보충 기록 용량은 1~8이다. polling 또는 deferrable을 선택하려면 정책명을 바꾸고 --server-credits를 생략한다.

~~~sh
python3 tools/build_native_el0.py --out ss --policy sporadic --server-slot-ms 50 --server-budget 2,5 --server-credits 2 --elf service.elf --peer-elf background.elf
~~~

[SDK build / QEMU 실행](../examples/native-aperiodic/README.md)

## 가중 앱이 단독 실행 중에도 퀀텀 유지

단일 CPU virt에서 --weighted-continuous를 지정하면 실행 가능한 앱이 하나여도 퀀텀 타이머를 유지한다. 이 옵션을 생략하면 기존 가중 정책에서는 단독 실행이 계속된다. 각 ELF에 양의 가중치를 지정한다. 퀀텀의 단위는 타이머 틱이다.

~~~sh
python3 tools/build_native_el0.py --out share --policy weighted --weighted-continuous --weighted-notify closures:36 --quantum-ticks 625000 --elf monitor.elf --weight 1 --arg A --arg share --peer-elf monitor.elf --peer-weight 2 --peer-arg B --peer-arg share
~~~

선택 사항인 --weighted-notify closures:N은 타이머 IRQ로 끝난 전체 퀀텀 N개, wakes:N은 실제 차단 해제 N회를 센다. 통지를 설정한 앱은 `<jnox/weighted_notify.h>`의 `jnox_weighted_notification()`으로 읽기 전용 값을 확인할 수 있다. 값은 0에서 1로 한 번 바뀐다. 통지를 받은 뒤에도 각 앱은 스스로 종료할 때까지 계속 실행된다.

[Archive extraction and QEMU commands / 원본 추출 및 실행](../examples/native-weighted/README.md#original-weighted-monitor--원래-가중치-모니터) · [#327](native-weighted-continuous.en-ko.md)

## 텔레메트리 게시 기한과 결과 읽기

MLQ 또는 MLFQ로 report.elf와 crc.elf를 패키징할 때 기자 앱의 인자로 --arg=--publication-deadline-ms=40을 전달한다. 각 회차의 요청 시각에서 완료된 경보를 게시한 뒤 얻은 표본까지 40 ms를 적용한다. 보고 주기는 그대로 40 ms다. 인자를 생략하면 기존 무기한 판정 모드로 실행한다. 정상 완료는 53, 잘못된 설정은 게시 전 106, 기한 초과는 결과와 표본을 남긴 뒤 107이다.

패키지와 해당 부팅의 캡처 디렉터리를 판독기에 전달한다. JSON에는 회차별 게시 지연과 여유 시간(ns), 경보 수, 백그라운드 CRC 결과가 들어 있다.

~~~sh
python3 tools/read_multilevel_telemetry.py "$pkg" "$dir" > "$dir/results.json"
~~~

[Bash / SDK / QEMU](../examples/native-multilevel/README.md#m4-telemetry-deployment--m4-경보-게시-배포)

## 프로세스별 계층형 CPU 예산 설정

단일 CPU virt에서 --policy hierarchy와 --hierarchy로 JSON 파일을 지정한다. ELF를 넣은 순서가 리프 번호가 된다. period_ticks와 예산은 카운터 틱 단위다. 두 리프가 같은 부모에 속하면 그 부모의 예산을 공유한다. 아래 명령은 예제에서 빌드한 CRC, 히스토그램, 에너지 앱 순으로 배치한다.

~~~sh
out=output/native-hierarchy
python3 tools/build_native_el0.py --out "$out/saturated" --policy hierarchy --hierarchy examples/native-hierarchy/baseline.json --elf "$out/crc.elf" --peer-elf "$out/histogram.elf" --peer-elf "$out/energy.elf"
~~~

단일 프로세스 대조에서는 같은 CRC ELF만 지정한다. 62.5 MHz에서 15,625,000틱 주기마다 리프에 3,906,250틱, 부모와 자식에는 한 주기씩 배정한다.

~~~sh
printf '%s\n' '{"period_ticks":15625000,"parents":[15625000],"children":[[0,15625000]],"leaves":[[0,3906250]]}' > "$out/single.json"
python3 tools/build_native_el0.py --out "$out/single" --policy hierarchy --hierarchy "$out/single.json" --elf "$out/crc.elf"
~~~

[SDK / QEMU](../examples/native-hierarchy/README.md) · [Single leaf / 단일 리프](native-hierarchical-applications.en-ko.md#single-leaf-control--단일-리프-대조)

선택형 공학 구성이다. 최초 10건의 산출물·원장은 일치했지만 포화·교환 일반 부팅의 유휴 타이머 지연이 기준을 넘었다.
 [EVD-135](evidence/evd-135.md)

## 네 스레드의 우선순위와 뮤텍스 설정

단일 CPU virt에서 한 프로세스의 메인과 조인 가능한 작업자 셋을 패키징한다. --thread-priorities는 슬롯 0~3의 기본 우선순위(1~31, 큰 수 우선)를 지정한다. 기존 --threads 2는 협력형 또는 동순위 RR 동작을 유지한다.

~~~sh
mkdir -p output/priority
sh sdk/native-aarch64/build-rust.sh examples/native-priority-mutex/Cargo.toml output/priority/priority.elf
python3 tools/build_native_el0.py --out output/priority/chain --elf output/priority/priority.elf --arg chain --threads 4 --policy fifo --thread-priorities 1,2,3,4
~~~

C의 pthread_attr_t와 Rust pthread 모듈의 PthreadAttr로 작업자를 설정한다. pthread_attr_setinheritsched, pthread_attr_setschedpolicy, pthread_attr_setschedparam으로 명시적 정책·우선순위를 선택한다. 뮤텍스 속성(pthread_mutexattr_t / PthreadMutexAttr)의 기본 프로토콜은 PTHREAD_PRIO_NONE이다. pthread_mutexattr_setprotocol으로 PTHREAD_PRIO_INHERIT 또는 PTHREAD_PRIO_PROTECT를 지정하고 즉시 천장은 pthread_mutexattr_setprioceiling으로 설정한다. jnox_mutexattr_setusers는 참여 슬롯 마스크를 설정하는 확장이다. pthread_mutex_clocklock에 CLOCK_MONOTONIC과 절대 timespec을 넘겨 기한까지 대기한다. 실물 M3 최장 차단 시간 및 제품별 천장 승인은 별도 판정 대상이다.

[C / Rust / QEMU](../examples/native-priority-mutex/README.md) · [POL-014](native-thread-priority-protocols.en-ko.md)
