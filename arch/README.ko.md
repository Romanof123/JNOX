# AArch64 커널 빌드

[English](README.md) · [시스템 아키텍처](../docs/architecture.md)

현재 구현된 백엔드는 AArch64 하나다. 저장소 루트에서 Python 3으로 아키텍처 목록을 확인하거나 고정 Rust 도구 체인으로 릴리스 커널을 빌드한다.

```sh
python tools/build_kernel.py --list-arches
python tools/build_kernel.py --arch aarch64
```

첫 명령은 Cargo를 실행하지 않고 `aarch64`를 출력한다. 둘째 명령은 `aarch64-unknown-none`용 커널을 빌드한다. 선택 우선순위는 `--arch`, `JNOX_ARCH`, 기본값 `aarch64` 순이다. 미지원 이름이나 충돌하는 `CARGO_BUILD_TARGET`을 지정하면 Cargo 시작 전에 오류로 종료한다.

`JNOX_BOARD`로 머신, `JNOX_PROFILE`로 런타임 프로파일, `JNOX_APP`으로 작업을 선택한다. 실행기는 이 환경값을 Cargo에 전달한다. 다음은 POSIX 셸 예시다.

```sh
JNOX_BOARD=pi4 JNOX_PROFILE=fixed JNOX_APP=fixed-fifo python tools/build_kernel.py --arch aarch64
```

기본 아키텍처로 Cargo를 직접 실행하려면 저장소 루트에서 다음 명령을 사용한다. 보드·프로파일·응용 환경값은 같은 방식으로 적용된다.

```sh
cd kernel
cargo build --locked --release -j1
```

[타깃 명세](aarch64/target)에는 Rust 타깃 트리플이 있다. [Rust 모듈](aarch64/mod.rs)은 바이트 그대로 옮긴 [기동 어셈블리](aarch64/start.S)를 커널 진입점에 연결한다. 보드별 링커 배치와 장치는 보드 설정에서 선택한다.

백엔드를 추가하려면 실제 기동 코드를 구현하고 `arch/<name>/target`에 타깃을 등록한다. 해당 타깃 조건에 구현을 연결하고 일치하는 Cargo 타깃을 허용하며 보드의 링커 배치를 제공한다. 실행기는 발견한 모든 타깃 명세를 목록에 표시하므로, 명세를 공개하기 전에 해당 머신에서 진입점·적재 바이트·기동을 검증한다.

## 검증

직접 Cargo 빌드와 선택기 빌드에서 같은 기본 커널이 생성됐고, QEMU virt와 Pi 4 모델에서 모두 부팅됐다. 네 빌드 구성에서 이전 커널과 모든 적재 구간이 일치했다. 스케줄링 이미지 두 개는 적재 구간 밖에서 차이가 있었다. 선택기는 미지원 이름과 충돌하는 타깃을 거부했다. [CHG-074 독립 검증 기록](../docs/evidence/arch-base-first.md) · [REQ-008](../docs/requirements.md) · [DES-004](../docs/architecture.md) · [VER-008](../docs/verification-plan.md)
