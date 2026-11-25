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

호스트 시험 9개가 통과했다. EVD-014 보관 패키지를 쓴 파일 재생에서 두 바이트열과 서로 다른 실행 ID를 확인했다. 추가 바이트와 기존 출력 폴더도 검출했다. 실보드의 전원 재인가와 보드 식별은 독립 입회가 필요하다. [호스트 검증 기록](../evidence/scheduling-tools-host-review.json)

<a id="chg-062"></a>

## CHG-062 · 물리 UART 프로파일 확장 계획

현재 호스트 도구는 읽은 바이트를 단일 pi4-scheduler 전사와 비교한다. 포트 모드와 두 파일 재생 모드에 선택 인자 `--profile pi4-scheduler|pi4-fixed-fifo|pi4-fixed-rr`를 추가한다. 생략 시 기존 스케줄러 호출·출력 파일·종료 상태와 직접 관측 함수의 기본값을 보존한다. 고정 우선순위 이미지는 #71의 일반 비계측 Pi 빌드다. 실제 보드 승인은 [#34](https://github.com/Romanof123/JNOX/issues/34)와 [POL-002 / #21](../scheduling-release-cases.md)에서 추적한다.

**바이트 계약.** 공통 시작은 `JNOX PI4 BOOT\nJNOX PI4 READY\n`이다. 아래 사건마다 접두어, 공백, 사건 문자열과 LF 하나로 한 줄을 구성한다. 표에 명시한 전체 순서만 허용한다.

| 프로파일 | 접두어 | 순서 |
|---|---|---|
| pi4-scheduler | SCHED | LOW RUN → TIMER IRQ → HIGH RUN → LOW RESUMED → PASS |
| pi4-fixed-fifo | FIXED FIFO | START → A START → B YIELD → A RESUME → HIGH → A DONE → B SLEEP → B DONE |
| pi4-fixed-rr | FIXED RR | START → A START → HIGH → B START → A DONE → B DONE |
| pi4-fixed-rr, HIGH 먼저 | FIXED RR | START → HIGH → A START → B START → A DONE → B DONE |

시리얼·파일 재생 및 qemu_boot의 세 Pi 프로파일이 하나의 크기 제한 바이트 비교기를 공유한다. 기존 TRANSCRIPTS 키·고정 작업 관측 API·다른 보드 계약을 보존한다. 고정 Pi 항목은 DynamicTranscript이므로 바이트 배열처럼 자를 수 없다. 일반 RR 부분 순서 검사는 HIGH → B START → A START도 받아들인다. Pi 고정 작업의 두 경로에서는 이를 거부한다. 임의의 읽기 분할에서 가능한 접두어만 유지하며 마지막 LF에서 완료를 인식한다. 불가능한 접두어나 추가 바이트는 즉시 거부하고 마지막 LF와 같은 읽기에 포함된 경우에도 실패 원문과 오프셋을 보존한다. 실제 포트와 파일 재생은 LF만 허용한다. QEMU 호출자가 정한 줄끝 옵션과 시간 처리 계약을 보존한다.

**입회와 출처.** 선택한 프로파일마다 같은 보드·카드·패키지로 전원을 완전히 제거한 뒤 두 번 독립적으로 입회 부팅한다. FIFO와 RR에는 각각 두 번이 필요하다. 전원이 꺼진 상태에서 관측을 시작하고 전원을 넣는다. 호스트 시작 후 30초 이내(경계 포함)에 마지막 LF가 도착해야 하며 이후 1초 동안 추가 수신이 없어야 한다. 실패·부분 캡처를 원래 ID로 유지하고 재시도는 새 디렉터리에 남긴다. 세션·실행 ID, 선언 프로파일, 실제 일치한 순서(실패 시 미일치), 출처 모드, 원문 해시, 읽기 시각·오프셋을 기록한다. 입회자가 두 전원 이벤트·보드 리비전·EEPROM·카드·어댑터 및 해시 대상이 마운트된 카드인지 스테이징 사본인지 명시한다. 선언한 빌드 옵션·리비전을 ELF, kernel8.img, config, 고정 펌웨어와 SHA256SUMS의 실제 해시로 연결한다. 파일명이나 UART 일치만으로 패키지 출처를 확정하지 않는다. 검증하지 못한 연결은 표시하고 모델 재생에는 보존된 패키지 정보만 사용한다.

**호스트 검증.** EVD-031 아카이브와 멤버 해시를 확인한 후 FIFO·RR 각 run-1/run-2 원본 UART를 대응 패키지·ELF 정보로 재생한다. 보존된 RR 두 건은 A START가 먼저다. 구성한 HIGH 우선 순서는 호스트 대조군으로 표시한다. EVD-014의 uart_hex와 패키지 정보로 스케줄러 명시·생략 호출을 확인한다. 같은 스케줄러 자료를 두 번 재생한 결과는 실행 ID 독립성만 검증한다. 바이트 단위·마커 중간 분할, RR 두 순서와 같은 쌍 내 다른 순서, 잘못된 프로파일·보드 태그, 마지막 LF 누락, CRLF, 순서 변경·중복·누락, 비정상 바이트, 동일 읽기 또는 정숙 구간의 추가 출력, 짧은 읽기와 모의 호스트 시계의 마감·정숙 경계를 검사한다. 분할·일괄 읽기의 판정과 원문을 비교한다. 물리 UART 항목은 입회한 실제 실행 후 판정한다. USB 타임스탬프와 모델 예외 로그로 물리 IRQ 위임·중단 PC·보드 지연·WCET를 확정할 수 없다. [EVD-031 원자료](../evidence/evd-031-raw.json) · [패키지 색인](../evidence/evd-031.json) · [EVD-014](../evidence/evd-014.json) · [고정 작업 가이드](pi4-fixed-runtime.md)

## 고정 정책의 물리 캡처와 파일 재생

`--profile pi4-fixed-fifo` 또는 `--profile pi4-fixed-rr`를 선택하고 해당 빌드의 부트 디렉터리와 ELF를 지정한다. 생략 시 기존 `pi4-scheduler`를 사용한다. 정책마다 새로운 출력 디렉터리에서 같은 보드와 카드로 두 번 완전히 전원을 분리한다. 두 번째 프롬프트에서도 전원을 끈 채 Enter를 누르고 “Armed”가 출력된 뒤 전원을 인가한다.

```sh
python3 tools/pi4_serial_capture.py --profile pi4-fixed-fifo --port /dev/ttyUSB0 --boot-dir /mnt/pi-boot --elf /evidence/fifo/kernel.elf --board-revision "$REV" --eeprom "$EEPROM" --card-id "$CARD" --adapter-id "$ADAPTER" --witness "$WITNESS" --boot-dir-source mounted-card --build-record /evidence/fifo-build.json --output fifo-physical-01

python3 tools/pi4_serial_capture.py --profile pi4-fixed-rr --replay /archive/rr/run-1/uart /archive/rr/run-2/uart --boot-dir /archive/rr/boot --elf /archive/rr/kernel.elf --boot-dir-source archive --output rr-replay-01
```

선택적 빌드 JSON 파일은 `profile`, `revision`, `build_options`, `elf_sha256`, 파일별 `files_sha256`, `package_manifest_sha256`, `firmware_commit`을 기록한다. 제출값과 호스트가 직접 계산한 해시를 대조하고 불일치를 세션에 기록한다. 해당 바이너리가 실제 부팅 카드에 쓰였는지는 실물 식별과 입회 기록이 필요하다. 자료에 보드 리비전·EEPROM 값이 없는 재생은 옵션을 생략하면 null로 남는다.

실행마다 세션·실행 ID, 요청 프로필, 정확히 일치한 순서, 첫 불일치 바이트의 오프셋, 호스트 준비 시각과 무응답 관측 시간을 남긴다. 전원 차단 확인과 실제 전원 인가의 입회 기록을 함께 보존한다. EVD-031 모델 및 EVD-014 파일 재생은 호스트 도구 검증용이다. 실물 FIFO/RR 확인은 [#34](https://github.com/Romanof123/JNOX/issues/34)에서 계속 추적한다.

통합 호스트 수집기는 시간 조건 시험 566건, 관찰 API 검사 8건, 모의 포트 사례 4건과 기존 캡처 시험 7건을 통과했다. [Pi 4 직렬 캡처 도구 · 호스트 검증](../evidence/pi4-fixed-serial-host.md)

## 실제 카드의 네이티브 FIFO/RR

[EVD-129 원본 ZIP](../evidence/pi4-native-fixed-applications-first.zip)을 빈 디렉터리에 푼다. 그 루트에서 `p=fifo` 또는 `p=rr`, `repo`를 저장소 경로로 설정한다. 해당 `d905/boot-$p/` 내용을 카드에 준비하고 `d905/$p/` 패키지 전체를 보존한다. 시작 전에 정책별 원본 매니페스트, 커널 ELF와 이미지, 순서가 정해진 앱 ELF 두 개, 펌웨어·설정·빌드 로그를 대조한다. 원래 빌드 경로는 출처 정보다. 재생에서는 불일치 때도 전체 제출 파일을 `replay-input.bin`에 보존한다. 각 결과는 완전한 기록, 남은 앞부분으로 뒷받침되는 관측, 판정 불가능을 구분한다. 두 최초 실행과 전원 입회 기록을 보존한다. UART만으로 복귀 PC/SP, 사적 메모리 결과, 보드 타이밍이나 실제 전원 재인가는 입증할 수 없다.

~~~sh
python3 "$repo/tools/pi4_serial_capture.py" --profile "pi4-native-$p" --native-manifest "d905/$p/manifest.json" --elf "d905/$p/kernel.elf" --boot-dir /mnt/pi-boot --port /dev/ttyUSB0 --board-revision "$REV" --eeprom "$EEPROM" --witness "$WITNESS" --card-id "$CARD" --adapter-id "$ADAPTER" --boot-dir-source mounted-card --output "physical-$p-01"
python3 "$repo/tools/pi4_serial_capture.py" --profile "pi4-native-$p" --native-manifest "d905/$p/manifest.json" --elf "d905/$p/kernel.elf" --boot-dir "d905/boot-$p" --replay /archive/run1/uart.bin /archive/run2/uart.bin --boot-dir-source archive --output "replay-$p-01"
~~~
