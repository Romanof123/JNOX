# Raspberry Pi 4의 고정 우선순위 Rust 태스크

[English](pi4-fixed-runtime.en.md)

CHG-033 · REQ-002/005/008/016/023 → DES-003/004/012 → IMP-031 → VER-002/005/008/016/023 → EVD-031. [#71](https://github.com/Romanof123/JNOX/issues/71)

코어 0은 0x80000에서 시작한다. 보조 코어는 공유 메모리에 접근하기 전에 대기한다. EL2 진입 코드는 비보안 EL1에 물리 카운터와 타이머 접근을 허용한다. EL1 직접 진입 시 로더가 접근 권한을 부여해야 한다. 기존 BCM2711 GIC 경로는 결합 EOI 모드에서 PPI 30을 설정한다. 실기기에서는 보안 펌웨어의 위임이 필요하다. 고정 우선순위 런타임은 virt와 동일한 일반 Rust 함수와 스택 프레임을 사용한다. Pi 고정 프로파일은 추가 Cargo 기능이 없는 비계측 FIFO와 RR만 허용한다. [보드 인터럽트 계약](../evidence/evd-014.md)

저장소 루트에서 QEMU 변수에 raspi4b를 지원하는 실행 파일 경로를 지정하고 실행한다. 패키징 도구가 boards/pi4/firmware.json의 고정 해시를 확인한다.

```sh
set -eu
out=$(mktemp -d)
for app in fixed-fifo fixed-rr; do
  (cd kernel && env -u JNOX_FIXED_TRACE JNOX_BOARD=pi4 JNOX_PROFILE=fixed JNOX_APP="$app" CARGO_TARGET_DIR="$out/$app/target" cargo build --release --locked -j1)
  elf="$out/$app/target/aarch64-unknown-none/release/jnox-kernel"
  aarch64-linux-gnu-objcopy -O binary "$elf" "$out/$app/kernel8.img"
  python3 tools/package_pi4.py --kernel "$out/$app/kernel8.img" --output "$out/$app/boot"
  (cd "$out/$app/boot" && sha256sum -c SHA256SUMS)
  sha256sum "$elf" "$out/$app/boot/kernel8.img"
  python3 tools/qemu_boot.py --profile "pi4-$app" --kernel "$out/$app/boot/kernel8.img" --qemu "$QEMU" --runs 2 --capture-dir "$out/$app/capture"
done
```

모든 UART 행은 정확히 한 번씩 LF로 끝나며 마지막 LF 이후 1초간 조용해야 한다. FIFO는 표시 순서를 따른다. 기존 RR 순서 계약은 HIGH가 A START 바로 앞에 오는 경우도 허용한다.

```text
JNOX PI4 BOOT
JNOX PI4 READY
FIXED FIFO START
FIXED FIFO A START
FIXED FIFO B YIELD
FIXED FIFO A RESUME
FIXED FIFO HIGH
FIXED FIFO A DONE
FIXED FIFO B SLEEP
FIXED FIFO B DONE
```

```text
JNOX PI4 BOOT
JNOX PI4 READY
FIXED RR START
FIXED RR A START
FIXED RR HIGH
FIXED RR B START
FIXED RR A DONE
FIXED RR B DONE
```

소스 리비전, ELF와 원시 이미지 해시, 에뮬레이터 버전, 패키지 이미지에서 수집한 UART 원본과 예외 추적을 보존한다. 같은 ELF로 IRQ와 SVC 진입 및 ERET 복귀 주소를 대조한다. 일반 Rust 함수의 정상 반환 후 종료 트램펄린 진입, 상위 우선순위 선점과 RR 동순위 교대를 확인한다. raspi4b에서 CNTFRQ_EL0를 읽어 틱을 시간으로 변환한다. 12,500,000틱은 virt의 62.5 MHz에서 200 ms이다. 호스트 경과 시간은 독립적으로 측정한다. 모델의 직접 로딩은 EEPROM과 VideoCore 펌웨어를 우회한다. 실기기 전원 재인가 2회, 실제 IRQ 위임, 보드 지연 측정, 시스템 안전 승인은 [#34](https://github.com/Romanof123/JNOX/issues/34)와 [#21](https://github.com/Romanof123/JNOX/issues/21)에서 별도로 추적한다.
