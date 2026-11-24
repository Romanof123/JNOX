# Pi 4 스케줄러 부팅 두 차례 기록하기

[English](pi4-serial-capture.en.md) · [실물 검증 이슈 #34](https://github.com/Romanof123/JNOX/issues/34)

1. [타이머 패키지 안내](pi4-timer.md)에 따라 스케줄러 이미지와 SD 카드를 준비한다. 호스트에 카드를 마운트하고 부팅 디렉터리, 이미지에 대응하는 ELF, 보드 리비전과 EEPROM 버전을 확인한다. [3.3 V UART 배선 안내](pi4-p1.md)에 따라 Pi TX와 어댑터 RX, 양쪽 GND만 연결한다. 전원선과 DTR·RTS는 연결하지 않는다.
2. 실물 캡처에는 호스트 Python 환경의 pyserial이 필요하다(`python3 -m pip install pyserial`). 저장소 루트에서 아래 명령을 실행한다. 장치명(Windows 예: `COM3`), 경로, 리비전, EEPROM 버전을 실측 대상에 맞게 바꾼다. 출력 경로는 기존 상위 디렉터리 아래에 새 이름으로 지정한다.

```sh
python3 tools/pi4_serial_capture.py --port /dev/ttyUSB0 --boot-dir /mnt/pi-boot --elf kernel/target/aarch64-unknown-none/release/jnox-kernel --board-revision REV --eeprom VERSION --output pi4-physical-01
```

3. 도구는 첫 안내 문구를 표시하기 전에 부팅 파일과 ELF를 읽고 패키지 해시를 대조한 뒤 세션 정보를 기록한다. 마운트한 카드를 안전하게 꺼내 전원이 분리된 Pi에 장착한다. 준비용 디렉터리 사본의 해시를 사용했다면 카드 내용도 별도로 확인하고 어느 쪽을 해시했는지 기록한다.
4. 매 회차 보드 전원을 완전히 분리한다. 첫 안내에서 전원이 꺼진 상태로 Enter를 누르면 도구가 115200 8N1로 어댑터를 열고 이전 수신 버퍼를 비운다. 다음 안내에서 Enter를 누른 직후 작업자가 직접 전원을 연결한다. 2회차에도 같은 보드와 카드를 유지하고 전원을 다시 완전히 분리한다. 도구는 UART를 읽고 호스트에 파일을 쓰며, 전원 조작은 작업자가 수행한다.
5. 두 `run-*/result.json`을 확인한다. 일치 판정에는 호스트의 캡처 시작 후 30초 이내에 LF 줄바꿈을 포함한 정확한 스케줄러 출력이 수신되고 이후 1초간 추가 바이트가 없어야 한다. 각 회차의 `uart.bin`에는 원시 바이트, `reads.jsonl`에는 읽기별 바이트 오프셋과 호스트 시각이 들어간다. 서명 전 상태의 `session.json`에는 파일 해시와 작업자가 입력한 보드 정보가 있다. 중단된 디렉터리도 보존하고 재시도에는 새 이름을 쓴다. 입회자, 보드·카드 식별 정보, 두 전원 조작 시점, 이미지와 ELF의 출처를 증거와 함께 기록한다. 중단된 명령 주소나 하드웨어 타이밍을 입증하려면 별도 계측이 필요하다.

## 파일 재생 및 호스트 테스트

일반 파일 두 개를 재생하면 각 결과에 `file_replay`가 기록된다. 여기서 측정한 시간은 호스트의 파일 읽기 시간이다. 파일 재생과 모의 입력 테스트는 Python 표준 라이브러리만 사용한다.

```sh
python3 tools/pi4_serial_capture.py --replay first.bin second.bin --boot-dir pi4-scheduler-boot --elf kernel/target/aarch64-unknown-none/release/jnox-kernel --board-revision replay --eeprom replay --output pi4-replay-01
python3 -m unittest discover -s tools -p "test_pi4_serial_capture.py"
```
