# 커널 아키텍처 선택

[English](README.md) · [시스템 아키텍처](../docs/architecture.md)

CHG-074는 기존 AArch64 커널의 빌드 인터페이스를 정의한다. 루트 선택기와 소스 배치는 구현을 위한 명세다. 이번 변경의 빌드·부팅 검증은 대기 중이다.

저장소 루트에서 등록된 백엔드를 나열하거나 AArch64 커널을 빌드한다.

```sh
python tools/build_kernel.py --list-arches
python tools/build_kernel.py --arch aarch64
```

목록 명령은 `aarch64`만 출력하고 Cargo 실행 없이 종료한다. --arch 생략 시 JNOX_ARCH를 사용하고 환경값도 없으면 aarch64를 기본값으로 쓴다. CARGO_BUILD_TARGET이 충돌하면 Cargo 시작 전에 거부한다. 선택 빌드는 기존 `aarch64-unknown-none` 타깃과 Cortex-A72 컴파일러 설정을 사용한다. `kernel/`에서 직접 실행하는 Cargo는 기존 기본값을 사용한다. 두 경로 모두 `JNOX_BOARD`, `JNOX_PROFILE`, `JNOX_APP`의 보드·프로파일·응용 선택을 유지한다. 보드가 아키텍처와 독립적으로 링커 배치와 장치를 선택한다. [Cargo 설정](../kernel/.cargo/config.toml)

나열되는 각 백엔드에는 이름·Rust 타깃·구현 소스를 식별하는 단일 등록 명세가 필요하다. 최초의 유일한 백엔드인 `arch/aarch64/`는 실제 커널에 링크되는 기동 어셈블리 또는 초기 CPU 훅을 제공한다. 목록과 선택은 동일한 등록 정보를 사용한다. 디렉터리 이름만으로 등록하지 않는다. 알 수 없는 아키텍처나 명시적 타깃·환경 타깃과의 충돌은 Cargo 시작 전에 지원 이름 또는 기대 트리플을 안내하고 0이 아닌 값으로 종료한다. 직접 Cargo 빌드도 링크 전 실제 타깃을 확인한다.

VER-008에서는 동일한 도구·보드·응용 입력으로 이전 기준 이미지와 직접 Cargo·선택기 빌드를 QEMU virt 및 Pi 4 모델에서 비교한다. 실제 타깃, 링크 진입점, 적재 범위·바이트, Pi 원시 이미지와 기존 기동 관찰을 기록하고 기준과의 차이를 설명한다. 목록, 미지원 이름 및 타깃 거부, 거부 시 Cargo 미실행을 독립 확인한다. 검증 범위는 실행한 모델 구성이다. [REQ-008](../docs/requirements.md) · [DES-004](../docs/architecture.md) · [VER-008](../docs/verification-plan.md)
