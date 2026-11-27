# J-NOX 개발자 가이드

[English](developer-guide.en.md) · 한국어

C 앱과 Rust 앱을 하나의 AArch64 커널에 패키징하고 UART에서 각 프로세스의 종료 상태를 확인한다. 이어서 Rust 드라이버를 개발하거나 원본 Linux C 드라이버를 이식하는 경로를 설명한다. 아래 명령은 Linux 또는 WSL에서 공개 J-NOX 커밋 `5dabcd904a83b0301650c971d4eba4e884fd1eb5`를 사용한다. 실행 대상은 QEMU TCG의 Cortex-A72 한 개, GICv2, 메모리 128 MiB, 네이티브 FIFO 스케줄러다. [기준 소스](https://github.com/Romanof123/JNOX/tree/5dabcd904a83b0301650c971d4eba4e884fd1eb5)

## 빌드 환경 준비

Bash, Git, Python 3.10 이상, rustup, AArch64를 지원하는 Clang, LLD, AArch64 크로스 도구를 포함한 GNU binutils, GNU coreutils, `qemu-system-aarch64`를 준비한다. RTC 예제에는 curl과 Python 패키지 pyelftools도 필요하다. 이후 명령은 같은 셸의 저장소 루트에서 실행한다. 각 패키지의 출력 디렉터리는 새 경로여야 한다. [커널 도구 버전](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/kernel/rust-toolchain.toml) · [Rust SDK 빌드 스크립트](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/sdk/native-aarch64/build-rust.sh)

```sh
git clone https://github.com/Romanof123/JNOX.git jnox-guide
cd jnox-guide
git fetch origin refs/pull/458/head
git checkout --detach 5dabcd904a83b0301650c971d4eba4e884fd1eb5
rustup toolchain install 1.95.0 --profile minimal
export RUSTUP_TOOLCHAIN=1.95.0
rustup target add aarch64-unknown-none aarch64-unknown-none-softfloat
work=$(mktemp -d)
printf '%s\n' "$work"
```

## 앱 두 개 패키징하기

SDK는 시작 어셈블리, 링커 스크립트, C 헤더와 Rust 바인딩을 제공한다. C 앱은 `main`에서 정수를 반환한다. Rust 예제는 `#![no_std]`, `#![no_main]`, C ABI로 공개한 `main`과 패닉 핸들러를 사용한다. 새 Rust 앱은 예제의 Cargo 설정을 따라 저장소 안의 `jnox-native` 크레이트를 참조하면 된다. 두 앱 모두 센서 샘플 네 개를 처리할 때마다 CPU를 양보한다. 패키저가 프로세스별 실행 이미지, 인자 배열, 우선순위를 설정한다. [C 예제](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/examples/native-argv/sensor.c) · [Rust 예제](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/examples/native-argv-rust/src/main.rs) · [Cargo 설정](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/examples/native-argv-rust/Cargo.toml)

```sh
set -eu
sh sdk/native-aarch64/build.sh examples/native-argv/sensor.c "$work/sensor-c.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-argv-rust/Cargo.toml "$work/sensor-rs.elf"
python3 tools/build_native_el0.py --board virt --kernel-profile release \
  --policy fifo --priority 1 --peer-priority 7 --out "$work/pair" \
  --elf "$work/sensor-c.elf" --arg sum --arg 16 --arg 32 --arg west \
  --peer-elf "$work/sensor-rs.elf" --peer-arg sum --peer-arg 240 --peer-arg 256 --peer-arg east
```

슬롯 0은 16부터 31까지 더해 376을 얻고, 슬롯 1은 240부터 255까지 더해 3960을 얻는다. 각 앱은 합계를 검사한 뒤 종료 코드 17과 43을 반환한다. 패키지의 `manifest.json`에는 슬롯별 입력 ELF, 해시, 인자와 우선순위, 커널 빌드 명령과 커널 해시가 기록된다. [인자 ABI와 기댓값](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/examples/native-argv/README.md)

## 부팅 후 결과 확인

```sh
status=0
timeout 20s qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 \
  -smp 1 -accel tcg -m 128M -nodefaults -display none -monitor none -no-reboot \
  -serial "file:$work/uart.log" -kernel "$work/pair/kernel.elf" || status=$?
test "$status" -eq 0 || test "$status" -eq 124
tr -d '\r' < "$work/uart.log" > "$work/uart.txt"
grep -Fx 'NATIVE EL0 EXIT PID 0 17' "$work/uart.txt"
grep -Fx 'NATIVE EL0 EXIT PID 1 43' "$work/uart.txt"
```

커널은 두 종료 상태를 출력하고 대기한다. 호스트의 timeout은 실행 시간을 제한한다. 두 UART 행이 모두 일치해야 한다. 원본 UART와 매니페스트를 보관한다. 계산 결과까지 독립적으로 확인하려면 각 프로세스의 활성 주소 공간을 사용해 `sensor_sum`을 읽고 위 합계와 대조한다. 같은 가상 주소가 다른 프로세스에서 무엇을 가리키는지는 해당 주소 공간에 따라 달라진다. [네이티브 캡처 도구](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/tools/native_el0_capture.py)

## 네이티브 Rust 드라이버 개발

먼저 장치 식별자, 소유 슬롯, 레지스터 범위, IRQ, DMA 한도, 리셋 완료 조건과 한 번에 처리할 최대 작업량을 정한다. EL0 서비스와 EL1 코드가 각각 어떤 작업을 수행할지도 명시한다. 이 커밋의 virtio-net 프로필에서는 EL1의 Rust 코드가 MMIO와 고정 DMA 큐를 다룬다. EL0의 Rust IPv4 서비스는 SDK가 권한을 검사하는 프레임 입출력 연산을 통해 패킷과 소켓 엔드포인트를 처리한다. 이 경계를 기준으로 권한 부여와 비인가 호출 시험을 설계한다. [Rust 전송 계층](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/kernel/src/native_virtio_net.rs) · [EL0 IPv4 서비스](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/services/inet/README.md) · [설계 DES-009](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/docs/architecture.en.md)

첫 장치 실습으로는 PL011 수신 IRQ 예제가 작다. EL0 Rust 앱이 디스플레이 소유자 API로 수신 바이트와 IRQ 통계를 읽는다. 소유자에 `--display-service 0 --uart-rx-irq`를 설정하고 두 번째 슬롯에 `denied` 인자를 준다. 검증 도구가 8바이트를 보낸 뒤 16바이트 링에 24바이트를 보내고, 앱이 화면 첫 줄에 기록한 결과를 독립적으로 읽는다. 폴링 패키지와 화면 없는 패키지까지 확인하면 가상 장치의 인터럽트 전달, 초과 입력, 접근 거부를 구분할 수 있다. [세 패키지 빌드·실행 절차](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/examples/native-uart-rx-irq/README.md) · [EL1 IRQ 처리 코드](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/kernel/src/native_uart_rx.rs)

## Linux C 드라이버 원본 이식

PL031은 Linux v6.18 커밋 `7d0a66e4bb9081d75c82ec4957c50034cb0ea449`의 C 파일을 그대로 사용한다. 가져오기 도구는 파일 크기, SHA-256, Git blob ID와 SPDX 선언을 확인한다. 지원되는 Linux API를 구현한 어댑터를 사용해 원본을 별도 오브젝트로 컴파일하고 외부 심볼을 검사한 뒤 정적 J-NOX 서비스 ELF로 링크한다. 추가로 지원할 API는 어댑터에서 구현한다. [PL031 원본](https://github.com/torvalds/linux/blob/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/rtc/rtc-pl031.c) · [가져오기 프로필](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/drivers/linux-compat/profiles/rtc-pl031.json)

```sh
curl -fL https://raw.githubusercontent.com/torvalds/linux/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/rtc/rtc-pl031.c \
  -o "$work/rtc-pl031.c"
python3 tools/import_linux_driver.py --profile rtc-pl031 --source "$work/rtc-pl031.c" --out "$work/import"
sh sdk/native-aarch64/build.sh examples/linux-source-rtc/peer.c "$work/rtc-peer.elf"
python3 tools/build_native_el0.py --board virt --linux-source-rtc --policy fifo \
  --priority 8 --peer-priority 8 --elf "$work/import/service.elf" \
  --peer-elf "$work/rtc-peer.elf" --out "$work/rtc"
python3 tools/verify_linux_source_rtc.py --package "$work/rtc" \
  --driver-manifest "$work/import/import.json" --out "$work/rtc-run"
python3 tools/verify_linux_source_rtc.py --replay --require-gic --package "$work/rtc" \
  --driver-manifest "$work/import/import.json" --out "$work/rtc-run"
```

재검증 도구는 소스와 ELF의 식별값, RTC 레지스터 접근, GIC 응답, 비인가 프로세스의 접근 거부와 정리를 확인한다. 네트워크에는 별도의 virtio-net 빌더와 지정된 AArch64 헤더를 사용한다. 원본은 Linux stable v6.18.65 커밋 `4700e3adf3b24b39dd4929e9c6d62811ba3ccea8`의 `drivers/net/virtio_net.c`다. RX IRQ 또는 서비스 재시작 등 변경할 동작을 실행하는 프로필을 고른다. 헤더 패키지와 구성 해시, 오브젝트, 서비스 ELF를 함께 보관한다. [RTC 검증 기록과 명령](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/docs/evidence/native-linux-source-rtc/README.md) · [virtio-net 원본](https://github.com/gregkh/linux/blob/4700e3adf3b24b39dd4929e9c6d62811ba3ccea8/drivers/net/virtio_net.c) · [네트워크 프로필](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/drivers/linux-compat/profiles/virtio-net.json) · [빌더](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/tools/build_linux_source_virtio_net.py)

## 변경에서 릴리스까지

1. **요구사항을 정한다.** REQ와 CHG, 대상 보드와 역할, 외부에서 확인할 결과 및 실패 조건을 기록한다. 시간 기준과 측정 시계는 측정 전에 정한다.
2. **설계를 기록한다.** DES에 권한 경계, 메시지 또는 레지스터 ABI, 큐 한도, 소유권, 재시작 정책을 연결한다. 이전 세대의 핸들·대기 IRQ·DMA 버퍼가 언제 무효가 되는지 정한다.
3. **구현하고 패키징한다.** IMP와 구현 커밋을 연결한다. 원본과 라이선스, 컴파일러 버전, 전체 명령, 매니페스트, 산출물 해시를 남긴다. 기준 프로필과 변경 프로필은 별도 디렉터리에 빌드한다.
4. **독립적으로 검증한다.** 외부 패킷, 레지스터 변화, 화면 또는 마감 시각 열처럼 VER 사례에서 직접 관찰할 값을 정한다. 무권한 호출이나 변조된 재생 입력을 포함한다. 독립 검토자가 원시 기록과 ELF 식별값을 대조하고, 각 실행에서 확인한 범위를 기록한다.
5. **증거를 릴리스한다.** 추적표에 REQ, DES, IMP, VER, EVD를 연결한다. 원시 캡처, 해시, 재생 방법, 알려진 한계를 묶고 새 디렉터리에 풀어 재검증한 결과를 남긴다. QEMU에서 측정한 결과는 해당 가상 머신 조건과 함께 보고한다. [요구사항](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/docs/requirements.md) · [추적표](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/trace/links.csv) · [PL031 릴리스 사례](https://github.com/Romanof123/JNOX/blob/5dabcd904a83b0301650c971d4eba4e884fd1eb5/docs/evidence/native-linux-source-rtc/README.md)

PM은 변경 이슈와 PR을 서로 연결한다. 동작을 바꿀 때에는 시스템팀이 영향받는 가장 상위 요구사항부터 다시 설계하고, 기능안전팀은 안전 한도의 근거를 정한다. 개발팀은 병렬로 작업할 수 있다. 통합 빌드·게스트 시험·공식 검토는 한 번에 하나씩 진행한다. 처음 검증하는 담당자에게는 계약, 후보 버전과 원본 입력을 먼저 제공하고 이전 판정은 독립 판정 뒤에 공유한다. 배포팀은 검증한 구성과 남은 한계를 기록한다. SDK 호출, 빌드 명령, 예상 결과가 바뀌면 두 언어판을 함께 갱신한다. [PR 양식](../.github/pull_request_template.md)

파일시스템 작업은 패키지 파일 예제부터 시작한다. 읽기 전용 루트, 휘발성 `/tmp`, 영속 매체와 복구의 합격 기준은 단계별로 확인할 수 있다. [CAP-007](filesystem-plan.md) · [#459](https://github.com/Romanof123/JNOX/issues/459)
