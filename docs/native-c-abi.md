# 네이티브 AArch64 C 응용 ABI v1

[English](native-c-abi.en.md) · CHG-086 · #21 및 #24 부트스트랩 설계안 · [POSIX 소스 프로파일](posix-profile.md) · [사용자 프로그램 아키텍처](architecture.md)

첫 응용 예제는 실행 인자로 파일 경로 하나를 받고 십진 센서 기록을 읽는다. 요약은 파일 서술자 1, 오류 메시지는 2에 쓰고 main의 반환값을 종료 상태로 사용한다. 같은 C 소스를 대상으로 ELF 생성, EL0 진입, 파일 서비스 동작과 응용 종료를 각각 검증한다.

## 실행 파일과 초기 스택

리틀엔디언 AArch64 LP64, AAPCS64 정수 호출 규약, 정적 ET_EXEC를 사용한다. Clang의 aarch64-none-elf 타깃과 LLD로 링크한다. 파일 바이트가 있는 RX 세그먼트의 첫 주소와 _start는 0x00400000이다. VA [0x00400000,0x00800000)에 필요한 R과 RW를 분리하여 최대 8개 LOAD를 허용한다. 파일 오프셋과 시작 주소를 4096바이트로 정렬하고 페이지 간 중복 및 쓰기·실행 동시 권한을 금지한다. BSS를 포함해 메모리에만 있는 꼬리 영역은 0으로 초기화한다. 런타임 재배치, 인터프리터, 동적 링크, TLS 세그먼트는 지원 범위 밖이다. SDK는 범용 레지스터만 사용한다.

신뢰된 실행 구성이 native v1을 선택한다. 읽기 전용 LOAD의 SHF_ALLOC SHT_NOTE 섹션 .note.jnox.abi를 확인한다. 노트는 리틀엔디언 namesz=5, descsz=4, type=1, 소유자 JNOX와 NUL, 0 패딩 3바이트, u32 버전 1로 구성된다. 별도 PT_NOTE는 필요 없다. 실행 권한은 출처 검증, 로더 검증 및 프로세스 권한 검사로 부여한다. 설정과 ABI가 다르면 진입 전에 거절한다.

로더는 이미지 밖에 쓰기 가능하고 실행 불가능한 스택을 준비한다. SP는 16바이트 경계이고 첫 u64는 0..INT_MAX 범위의 argc다. SP+8에 argc개의 NUL 종료 문자열 포인터를 놓고 마지막에 널 포인터를 붙인다. 실행기는 개수, 주소와 문자열 범위를 검사한다. envp와 auxv는 사용하지 않는다. 시작 코드는 x0, x1로 argc, argv를 main에 전달하고 int 결과를 부호 확장해 exit을 요청한다. 종료 후 복귀하면 예외를 발생시킨다. 현 프로파일은 주소 공간마다 단일 스레드만 허용한다. 스레드별 errno 및 문맥 규약을 구현할 때까지 두 번째 스레드 생성을 거부해야 한다. 주소를 반환하는 errno 접근자는 추후 스레드별 저장소를 선택할 수 있다. 비동기 신호 처리의 안전성은 별도 검증 대상이다.

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
