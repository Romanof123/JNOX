# P1 실보드 부팅 벤치 · Raspberry Pi 4 Model B

한국어 · [English](pi4-p1.en.md)

저비용 P1 후보는 **Raspberry Pi 4 Model B 1 GB**다. BCM2711에 Arm Cortex-A72 코어 4개를 갖춘 보드다. 첫 관찰 대상은 코어 하나에서 전원을 새로 인가했을 때 나오는 직렬 출력이다. 현재 프로젝트 소유자가 보드를 보유하지 않아 실보드 결과는 대기 중이다. A72 QEMU `virt` 벤치는 ELF를 직접 적재하며, 별도의 메모리맵과 UART 계약을 사용한다. [보드 이슈 #34](https://github.com/Romanof123/JNOX/issues/34) · [제조사 사양](https://www.raspberrypi.com/products/raspberry-pi-4-model-b/specifications/) · [기존 가상 부팅 벤치](../m1-boot.md)

## 부팅 후보 만들기

CHG-013 · REQ-008/016 · DES-004/001 · IMP-011 · VER-008/016 · EVD-011. 펌웨어 태그 1.20250430과 SHA-256은 매니페스트에 고정했다. 전용 링커와 펌웨어는 원시 이미지 시작 주소를 `0x80000`으로 지정한다. 부트 코어는 EL2 또는 EL1로 진입해 EL1 스택을 설치한다. 다른 코어가 같은 진입점에 도달하면 대기한다. 저주소 주변장치 모드에서 PL011 주소는 `0xfe201000`, 입력은 48 MHz, 분주는 26 + 3/64다. IRQ·SMP 검증은 후속 단계다. [config.txt](../../boards/pi4/config.txt) · [Firmware](../../boards/pi4/firmware.json)

저장소 루트에서 Rust 1.95.0, aarch64-unknown-none 타깃, GNU AArch64 binutils와 Python 3를 사용한다.

```sh
(cd kernel && JNOX_BOARD=pi4 cargo build --release --locked -j1)
aarch64-linux-gnu-objcopy -O binary kernel/target/aarch64-unknown-none/release/jnox-kernel kernel8.img
python3 tools/package_pi4.py --kernel kernel8.img --output pi4-boot
(cd pi4-boot && sha256sum -c SHA256SUMS)
```

매니페스트에 고정된 공식 펌웨어를 내려받거나, 이미 받은 공식 boot 트리를 `--firmware-dir`로 지정한다. 새로 준비한 FAT32 부팅 파티션의 루트에 `pi4-boot/` **내부 파일과 overlays 디렉터리**를 복사한다. 카드 루트에서 `sha256sum -c SHA256SUMS`를 실행하고 카드를 안전하게 꺼낸다. 기본 Arm 스텁을 사용한다. UART의 EEPROM BOOT_UART와 펌웨어 uart_2ndstage 진단은 출력 판정 전에 꺼 둔다. Pi 4를 지원하는 QEMU에서는 아래처럼 원시 이미지를 직접 시험한다. 이 경로는 EEPROM, microSD, VideoCore 펌웨어를 건너뛴다. QEMU의 raspi4b 모델은 RAM 2 GiB다.

```sh
python3 tools/qemu_boot.py --profile pi4 --kernel kernel8.img
```

## 후보 인계

| 담당 | 검증자에게 후보를 전달하기 전에 고정할 항목 |
| --- | --- |
| 시스템·BSP | 보드 리비전과 1 GB 모델, EEPROM 부트로더 버전·설정, microSD 부팅 순서를 식별한다. Pi 4용 펌웨어와 대응 fixup 파일, 보드 DTB, 오버레이, `config.txt`를 포함한 FAT 부팅 파일과 해시를 고정한다. AArch64 모드와 전용 원시 이미지 파일명을 명시한다. 선택한 Arm 스텁, 실제 적재 주소, 링크 기준 주소, 진입점 오프셋, 사용 가능한 RAM과 예약 영역을 기록한다. 링크 맵과 펌웨어 설정의 일치를 확인한다. [부팅 파일](https://www.raspberrypi.com/documentation/computers/configuration.html#boot-folder-contents) · [부팅 설정과 Arm 스텁](https://www.raspberrypi.com/documentation/computers/config_txt.html#armstub) |
| BSP·커널 | 스텁이 명시한 진입 상태를 기록하고 진입 시 `CurrentEL`을 읽는다. 지원하는 각 진입 레벨에서 EL1에 도달하는 경로, 스택·BSS 초기화, 보조 코어 대기 방식을 명시한다. 첫 벤치는 PL011 UART0을 폴링 방식으로 구동하고 GPIO14로 송신한다. Pi 4의 기본 구성은 UART0을 블루투스에 배정한다. 핀 라우팅, GPIO 기능, CPU가 보는 MMIO 주소, UART 입력 클록과 115200보의 분주값을 고정한다. 보드 시작 코드에서 핀과 UART를 설정한다. IRQ 단계 전에는 타이머 소스, 인터럽트 컨트롤러 모드·주소·라우팅을 기록한다. [UART 라우팅](https://www.raspberrypi.com/documentation/computers/configuration.html#configure-uarts) · [BCM2711 주변장치](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf) |
| 통합 | 소스 리비전, 고정 툴체인, 빌드·패키징 명령, ELF·링크 맵, 원시 이미지와 SHA-256 명세를 제공한다. microSD 파티션 구성, 정확한 파일명, 복사 절차, 카드에서 다시 읽은 파일의 해시를 포함한다. 판정용 캡처에서는 같은 직렬 선로를 쓰는 펌웨어·부트로더 진단 출력을 끈다. 진단 실행 기록은 별도로 보존한다. |

## 첫 실보드 캡처

1. 지정 보드, microSD 카드와 리더기, 5 V / 3 A USB-C 전원, **3.3 V 로직** USB-UART 어댑터, 점퍼선 두 개를 준비한다. 보드 전원을 분리한 채 어댑터 GND를 헤더 **물리 핀 6(GND)**에 먼저 연결하고, 어댑터 RX를 **핀 8(GPIO14, Pi TX)**에 연결한다. 어댑터 TX와 VCC는 비워 둔다. Pi에는 USB-C로 전원을 공급한다. [제조사 헤더 핀맵](https://www.raspberrypi.com/documentation/computers/raspberry-pi.html#gpio) · [UART 전압·핀 배정](https://www.raspberrypi.com/documentation/computers/configuration.html#configure-uarts)
2. 카드 내용을 후보 명세와 대조한다. 전원이 꺼진 상태에서 **115200보, 데이터 8비트, 패리티 없음, 정지 1비트**로 어댑터를 열고 흐름 제어·에코·줄바꿈 변환을 끈다. 잔여 수신 데이터를 비운 뒤 새 바이너리 캡처를 시작하고 USB-C 전원 인가 시점을 호스트 단조 시계로 기록한다. 호스트 시각과 진단 출력은 별도 파일에 저장한다.
3. 수신 데이터는 아래 두 줄, 정확히 ASCII 29바이트여야 한다. 마지막 줄을 포함해 각 줄은 LF(`0a`) 하나로 끝난다. 두 줄 모두 부트 코어가 EL1에서 시작 코드가 오염시킨 BSS 검사 변수의 초기화를 확인한 뒤 같은 직렬 출력 경로에서 보낸다.

   ```text
   JNOX PI4 BOOT
   JNOX PI4 READY
   ```

   전원 인가 후 **30초** 이내에 마지막 LF가 수신되어야 한다. 그 직후 보드 전원과 수신기를 유지한 채 **1초** 동안 추가 바이트가 없는지 관찰한다. CR을 포함한 여분의 바이트, 순서 오류, 중복, 불완전한 출력, 시간 초과는 실패다. 수신기 연결이 끊기거나 전원 인가 시점을 관찰하지 못하면 해당 캡처는 무효다. 두 시간 값은 호스트 관찰 구간을 정한다.
4. 전원을 완전히 분리한다. 같은 카드와 이미지로 새 캡처를 시작해 반복하고 두 번의 콜드 부팅이 각각 통과해야 한다. 보드 리비전, EEPROM 설정, 카드 재읽기 해시, 전원, 어댑터·포트, 캡처 도구·버전, 회차별 전원·수신 시각, 원본 바이트, 관찰 종료 사유, 독립 판정을 저장한다. [검증 증거 양식](../templates/verification-evidence.md)

## 단계별 근거 범위

| 단계 | 수락할 수 있는 근거 |
| --- | --- |
| 호스트 이미지 | 재빌드 해시, 링크 맵, 원시 이미지, 카드 명세로 패키지의 동일성과 배치를 확인한다. |
| Pi 4 모델 확보 시 | 에뮬레이터 버전과 머신, 모델링한 펌웨어·주변장치, 생략한 경로, 원본 UART를 기록한다. 범용 `virt` 실행은 가상 P1 기준선에 기록한다. |
| 실보드 전원 인가 | 두 캡처는 기록한 Pi 리비전의 부팅·UART 계약을 입증한다. |
| 실보드 타이머·IRQ·SMP | 먼저 코어 하나에서 타이머 IRQ, 중단된 PC, 높은 우선순위 스레드의 실행과 원래 스레드로의 복귀를 식별한다. 이후 SMP 시험에서 보조 코어 시작, 코어별 ID, CPU 친화성, 경합 중 코어 간 깨우기를 확인한다. 판정은 각각 보존한다. |
| 제품 시간 한계·안전성 | 보드별 시간 한계를 배정하고 의도한 제품 구성에서 간섭을 분석하며 부하 중 시간을 측정한다. 기능안전팀과 통합팀이 상한, 가정, 승인 범위를 검토한다. P2 Cortex-R52는 별도 포트와 근거로 관리한다. [검증계획](../verification-plan.md) |

[EVD-011](../evidence/evd-011.md)

[타이머 선점 검증 구성](pi4-timer.md)
