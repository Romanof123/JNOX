# Pi 4 단일 코어 타이머 선점

[English](pi4-timer.en.md)

CHG-016 · REQ-002/005/008/023 → DES-003/004/012 → IMP-014 → VER-002/005/008/023 → EVD-014. [#41](https://github.com/Romanof123/JNOX/issues/41) · [#34](https://github.com/Romanof123/JNOX/issues/34)

코어 0에서 실행되는 LOW와 HIGH에 서로 다른 스택을 할당한다. LOW가 실행 중인 동안 비보안 물리 타이머가 GIC-400의 PPI 30을 발생시킨다. IRQ가 HIGH를 준비시키고 실제 고정 우선순위 스케줄러가 HIGH 프레임을 선택한다. HIGH의 SVC #1 종료 후 LOW는 중단된 명령으로 돌아가 상태를 확인한다.

저장소 루트에서 빌드하고 새로운 출력 디렉터리를 지정한다.

```sh
(cd kernel && JNOX_BOARD=pi4 cargo build --release --locked --features sched-bench -j1)
aarch64-linux-gnu-objcopy -O binary kernel/target/aarch64-unknown-none/release/jnox-kernel kernel8-sched.img
python3 tools/qemu_boot.py --profile pi4-scheduler --kernel kernel8-sched.img
python3 tools/package_pi4.py --kernel kernel8-sched.img --output pi4-scheduler-boot
(cd pi4-scheduler-boot && sha256sum -c SHA256SUMS)
```

공개 관찰기는 QEMU 실행 파일을 `--qemu`로 받을 수 있다. 최초 검증 시 해당 실행 파일에 `-d int -D "$TRACE"`를 추가한 래퍼를 전달한다.

먼저 구현을 보지 않은 검증자가 원시 이미지와 고정된 소스·ELF의 해시를 확인한다. QEMU raspi4b에서 한 번 실행해 103바이트(LF만 허용)와 마지막 LF 뒤 1초간 무출력을 확인하고 첫 판정을 기록한다. 동일 실행의 `-d int` 기록을 ELF에 대응해 타이머 IRQ와 SVC #1, 중단한 LOW의 PC, HIGH의 실제 진입, LOW 복귀 주소를 확인한다. IRQ 번호·타이머 레지스터와 상태 검사 근거를 외부에서 관측한 예외 주소와 구분한다.

```text
JNOX PI4 BOOT
JNOX PI4 READY
SCHED LOW RUN
SCHED TIMER IRQ
SCHED HIGH RUN
SCHED LOW RESUMED
SCHED PASS
```

실보드는 부트 펌웨어의 GIC 모드와 비보안 그룹 위임, 코어 0의 EL1 물리 카운터·타이머 접근 권한이 필요하다. 낮은 주변장치 주소 모드에서 GICD는 0xff841000, GICC는 0xff842000이다. 펌웨어와 EEPROM 버전, 실제 보드 리비전, raw/ELF 해시를 기록한다. 기존 Pi 4 직렬 연결 절차로 동일 SD 카드를 완전히 껐다 켜며 두 번 캡처한다. 각 회차 전원 인가 후 30초 내 아래 바이트를 모두 수신하고 1초간 무출력을 확인한다. 모델은 EEPROM과 VideoCore 펌웨어를 거치지 않는다. 이 출력만으로 실보드 IRQ의 중단 PC·서비스 지연을 측정한 것으로 보지 않는다. 실제 PC 대응에는 보드 추적·디버거 자료, 시간 한계 승인에는 별도 측정이 필요하다. [Pi 4 UART](pi4-p1.md)

[BCM2711 Arm stub](https://github.com/raspberrypi/tools/blob/master/armstubs/armstub8.S) · [BCM2711 timer / GIC](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/arch/arm/boot/dts/broadcom/bcm2711.dtsi) · [QEMU BCM2838 model](https://github.com/qemu/qemu/blob/master/hw/arm/bcm2838.c)

[EVD-014](../evidence/evd-014.md)
