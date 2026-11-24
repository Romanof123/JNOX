# 첫 AArch64 네이티브 프로세스 · 시작과 종료

[English](native-el0-startup.en.md) · CHG-087 · #8 · REQ-001/014/020 · 개발 검증 계획

독립적으로 링크한 정적 C 실행 파일을 Cortex-A72 한 개를 쓰는 virt 커널에서 AArch64 EL0로 진입시킨다. C 시작 코드가 사용자 스택의 인수를 읽고 main을 호출하며 main의 반환값을 네이티브 종료 서비스에 전달한다. 이 구성은 프로젝트가 작성한 계산 하나를 실행한다. 읽기 전용 파일시스템, 성공하는 표준 스트림 쓰기, 부모의 대기·종료 회수, 일반 프로세스 생성과 IPC는 미검증이다. #21 승인 뒤 #24의 IPC 구현을 시작한다. [네이티브 C 승인](posix-profile.md) · [파일시스템 설계](filesystem-plan.md)

**이미지와 실행.** 벤치마크 기능 없이 virt·native·startup-probe를 선택한다. CHG-086 SDK의 시작 코드·래퍼·링커로 `examples/portability/native_startup_probe.c`를 빌드해 ELF의 정확한 SHA-256을 기록한다. 읽기 전용 세그먼트의 할당된 네이티브 v1 ABI 노트를 선택한 실행 형식과 대조하고 CHG-085 로드 계획을 사용한다. 파일 바이트만 복사하고 BSS 등 남은 페이지는 0으로 만든다. [0x00400000,0x00800000)에 서로 분리한 사용자 RX·RO·RW 영역을, [0x01001000,0x01011000)에 실행할 수 없는 사용자 스택을 배치하고 양옆은 비워 둔다. 커널·페이지 테이블·동일 물리 메모리의 커널 별칭·UART·GIC는 특권 접근 전용이다. 128 MiB·단일 vCPU·virt GICv2를 고정한다. 이 초기 구동에서는 타이머·GIC를 정지하고 스케줄러 구성은 기존 IRQ 경로를 사용한다.

**초기 스택과 결과.** 16바이트 정렬 SP=0x01010fb0에 argc=3, `/bin/native_startup_probe`·`8`·`13` 포인터 셋, 0 두 워드를 둔다. 첫 0은 argv 종료다. 문자열은 SP+48·SP+74·SP+76에서 시작하고 80바이트 블록 마지막 바이트는 패딩이다. SDK 시작 코드는 argc와 argv를 읽는다. C 프로그램은 전달 인수, 초기화된 쓰기 가능 편향값 8, 처음에 0이어야 하는 누적값 둘을 확인한다. 읽기 전용 계수 2·1과 변환한 두 십진 인수로 16·13을 누적값에 쓰고 초기 편향값과 합산하면 37이다. `write(1,"37\n",3)`를 시도해 libc 반환 −1과 errno 38을 확인한 뒤 main의 결과를 SDK 종료 SVC로 넘긴다. 이번 부팅에서 구현한 서비스는 종료 하나다. 그 외 네이티브 ID의 원시 반환은 −38이며, 미지원 쓰기의 실제 응용 stdout은 0바이트다. 특권 UART는 프로세스가 넘긴 종료값을 기록한다.

**독립 판정.** 소스 커밋·빌드 인자·컴파일러/링커 버전·입력 ELF와 커널 ELF 및 해시·디스어셈블리·심벌·명령행을 고정한다. 첫 콜드 실행은 어느 종료 결과든 UART·QEMU 버전과 구성·같은 프로세스의 예외 추적·정지 메모리를 보존한다. (1) ELF 식별·적재 세그먼트·1단계 페이지 권한/속성·초기 스택, (2) EL1에서 SDK의 실제 엔트리로 가는 EL0t ERET, (3) x8=4와 사용자 버퍼·길이·바이트가 담긴 실제 write SVC, 원시 −38과 저장 사용자 PC로의 복귀, (4) x8=1·실제 x0=37의 exit SVC와 이후 사용자 복귀 없음까지 재구성한다. 두 예외에서 ESR_EL1.EC=0x15·즉시값 0x4a01, VBAR_EL1+0x400 하위 AArch64 동기 슬롯, 처리기의 실제 CurrentEL=4(EL1), SPSR_EL1.M=0(EL0t)가 필요하다. C 심벌 최종값과 남은 스택 인수를 독립 대조한다. 실패 또는 불확실한 판정도 남긴다. 이 구성의 결과 범위는 진입·메모리·종료이다. [Arm 예외 모델](https://developer.arm.com/-/media/Arm%20Developer%20Community/PDF/Learn%20the%20Architecture/Exception%20model.pdf) · [검증 계획](verification-plan.md)

`jnox_native_el0_image`에는 네이티브 ELF 전체가 들어간다. 리틀엔디언 `jnox_native_el0_journal`의 앞 16개 u64는 버전, 상태(1 준비·2 진입·3 종료·4 실패), 실패 코드, 이미지 길이, 엔트리, 초기 SP, 스택 하한·상한, 완료 이벤트 수, 부호 있는 원시 종료값, 종료값 유효 비트, stdout 바이트 수, 초기 CurrentEL·SCTLR_EL1·TTBR0_EL1, argc이다. 이어지는 최대 8행은 각각 u64 16개이며 인덱스, 벡터 오프셋, CurrentEL, ESR, SPSR, ELR, SP_EL0, x8, x0, x1, x2, 원시 반환(종료 시 0), 처리 결과(1 복귀·2 종료), VBAR, TTBR0, FAR 순이다. 완료 행만 읽고 아키텍처에서 유효하다고 정의한 예외에서만 FAR를 해석한다. 실측 종료값이 37과 달라도 기록한다.

의존 구현을 통합한 뒤 Linux 또는 WSL에서 `python3 tools/build_native_el0.py --out NEW_DIRECTORY`로 새 패키지를 만든다. C 이식 파일럿에는 실제 파일 서비스와 타깃 libc의 작동하는 stdout/stderr가 필요하다. 기존 Linux ELF와 네이티브 Rust SDK에도 별도의 승인 사례가 있다. [플랫폼 점검표](platform-checklist.md)

첫 독립 실행은 패키지·캡처용 새 디렉터리에서 한다. 전체 소스 커밋과 동일한 C 파일을 넘긴다. 재구성에는 캡처가 출력한 세션 SHA-256을 사용한다.

```sh
python3 tools/native_el0_capture.py NEW_CAPTURE_DIR NEW_DIRECTORY/kernel.elf NEW_DIRECTORY/user.elf examples/portability/native_startup_probe.c SOURCE_REV
python3 tools/native_el0_replay.py NEW_CAPTURE_DIR SESSION_SHA256
```

**첫 콜드 캡처 뒤 고정한 관측기 정정 기준.** 최초 캡처와 최초 재생 결과(종료 코드 1)를 보존한다. 커널은 네이티브 프로필 메시지 앞에 공통 특권 부팅 메시지를 출력했다. 원시 UART 전체가 LF로 끝나는 네 줄 `JNOX M1 BOOT`, `JNOX M1 READY`, `NATIVE EL0 ELF <사용자 ELF의 소문자 SHA-256>`, `NATIVE EL0 EXIT 37`과 순서대로 바이트 단위로 일치해야 한다. 누락·변경·순서 뒤바뀜·추가 바이트는 모두 실패이며, 응용 출력과 CRLF 치환도 여기에 포함된다. 정정한 관측기는 같은 최초 캡처에 적용한다.
