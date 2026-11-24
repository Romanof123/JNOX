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

Pi 4 모델에서 FIFO와 RR을 각각 두 번 부팅해 상위 우선순위 선점과 RR 동순위 교대를 확인했다. 별도 레지스터 관측에서 카운터 주파수는 62.5 MHz였다. 실보드 부팅 및 시간 검증은 남아 있다. [EVD-031](../evidence/evd-031.md)

CHG-062 · POL-002 · REQ/VER-002/005/008/016/023 · [Pi 4 물리 UART 프로파일 계획](pi4-serial-capture.md#chg-062)

<a id="chg-063"></a>

## CHG-063 · Pi 4 엔지니어링 이미지의 UART 실행 증거

[#34](https://github.com/Romanof123/JNOX/issues/34)의 POL-002에는 보드에서 어느 작업이 중단·재개됐는지 연결할 근거가 필요하다. 일반 앱은 A/B/HIGH의 SP와 진행량을 RAM에 기록하고 작업 표식을 UART로 출력한다. virt용 선택 고정 추적은 최대 64행×19워드로 입력·선택 프레임의 주소/ELR/SP, 원인, 소유자, 카운터와 타이머 상태를 기록한다. 별도 virt 비용 저널은 128행×7워드다. Pi 빌드는 두 옵션을 거부하고 RAM 관측은 정지된 에뮬레이터에 의존한다. 필요한 추가 기능은 Pi 시험 이미지 선택, 디스패치·재개 이후 작업 자체가 남긴 표본과의 인과 연결, 종료 후 제한된 내보내기다. EVD-031의 예외 PC는 모델이 기록했다. 당시 일반 UART에는 IRQ 프레임 PC가 없다. [모델 관측](../evidence/evd-031.md)

**이미지와 기록.** 단일 코어 Pi 4 fixed-fifo/fixed-rr만 전용 빌드 선택기를 허용한다. 타 보드·앱·벤치 기능·기존 fixed trace/cost와의 조합은 거부한다. 일반 빌드에서 훅·버퍼·내보내기를 제외하고 동일한 고정 일반 기준 ELF 및 로드 구간 해시를 비교한다. #141 일반 캡처의 바이트 순서와 마지막 정숙 구간을 유지한다. 시험 이미지에는 별도 캡처 프로파일과 스키마를 둔다. 기존 CPU별 제한 버퍼의 게시 방식과 역할·진행 관측을 재사용한다. 생성자는 커널과 링크된 예제 작업으로 제한하며 공개 추적 ABI나 비신뢰 생성자는 추가하지 않는다. [CHG-062 / #141](https://github.com/Romanof123/JNOX/issues/141) · [응용 추적 계약](../application-tracing.md)

부팅·세션 ID, 표, 실측 MPIDR 코어 식별자·현재 EL·CNTFRQ·원시 틱, 스키마·역할 매핑, 용량·게시 수·손실을 기록한다. A/B/HIGH의 실제 슬롯은 FIFO 0/1/2, RR 0/2/1이다. IRQ/SVC마다 승인값 또는 신드롬, 타이머 상태·비교값, 나가던 역할과 저장 프레임/ELR/SP, 선택한 역할과 프레임/ELR/SP, 디스패치 순번을 보존한다. 복원 전에 세대를 게시한다. HIGH·동급 작업의 최초 진입과 중단됐던 작업의 복귀 후 첫 진행에서 세대, 역할, 링크된 표본 PC, 실행 SP, 카운터, 작업 자체 진행량을 기록한다. 진입·후속 작업에서 횟수 제한 RAM 표본을 사용하고 프레임이 재사용되기 전에 원래 복귀점을 보존한다. 중간 선택 없이 직전 해당 디스패치와 연결해야 한다. 정확한 ELF·역어셈블리로 PC와 스택 범위를 판별한다. 최초 ELR은 진입 트램펄린일 수 있다. FIFO의 낮은 우선순위 A가 출력하는 A RESUME은 자발적 양보 다음이자 HIGH 이전이다. HIGH 종료 후 저장된 A 복귀점이 선택되고 A가 다시 진행한 근거를 요구한다. RR은 동급 작업의 실제 교대와 이후 A/B 진행을 관측한다. HIGH가 A의 진행 전에 실행되면 허용된 UART 순서는 보존하되 해당 부팅의 선점은 미관측으로 기록한다.

**내보내기와 판정.** 일반 작업 동작을 유지하고 사건 처리 중 UART, 동적 할당 및 문자열 변환을 추가하지 않는다. 필요한 작업과 마지막 종료, 실제 최종 유휴 진입 후 생성기를 정지시켜 헤더·행·무결성 확인용 푸터를 출력한다. 출력이 다시 추적 사건을 발생시키지 않아야 한다. 종료 누락·버퍼 초과·잘림·순서 변경·세대 불일치·부적절한 프레임/PC·진행 미확인·시각 모호성은 불완전 근거로 기록한다. 최초 실행, 실패와 원시 시리얼 바이트를 보존한다. 소스·설정·원시 kernel8.img·ELF 해시·로드 주소·맵/역어셈블리·펌웨어·실제 카드 패키지 매니페스트를 캡처와 연결한다. 보드·리비전·EEPROM·어댑터·입회자·완전 전원 차단 기록을 유지한다. 호스트 USB 시각은 출력 전송 관측이다.

raspi4b에서 일반·시험 FIFO/RR을 각각 연습하고 시험 기록을 같은 부팅의 독립 예외·벡터·ERET 로그와 대조한다. 다른 이미지·역할·세대, 행 누락·추가·손실을 파서 음성 대조로 사용한다. P1 보드에서는 명시한 보드·패키지에서 이미지·프로파일별 두 번의 독립 전원 차단 부팅에 입회한다. 모델과 보드 각각에서 일반·시험 이미지의 기능 순서와 결과를 비교한다. 시험 이미지의 관측 가능한 카운터 구간과 계측 비용을 측정하고 두 이미지에 공통 관측점이 있는 경우에만 차이를 계산한다. 메모리·스택·관측 위치·편차·순서 변화와 비교 불가능한 구간을 보고한다. 카운터가 없는 일반 UART에서는 무계측 서비스 시간의 수치 기준을 얻을 수 없다. 소프트웨어 인과 기록은 실제 진입과 재개 진행의 근거이고 저장·선택 ELR은 커널 기록이다. 물리 명령·ERET 이력이나 복귀에서 첫 명령까지의 정확한 지연에는 별도 하드웨어 추적·디버그 수단이 필요하다. 관측 범위를 넘는 IRQ 라우팅, 제품 한계 및 WCET는 별도 검증 대상이다. [REQ-002/005/008/016/023](../requirements.md) · [DES-003/004/012](../architecture.md) · [VER-002/005/008/016/023](../verification-plan.md)

통합 호스트 수집기는 시간 조건 시험 566건, 관찰 API 검사 8건, 모의 포트 사례 4건과 기존 캡처 시험 7건을 통과했다. [Pi 4 직렬 캡처 도구 · 호스트 검증](../evidence/pi4-fixed-serial-host.md)

### 계측 이미지 및 직렬 프로토콜

Pi 4 고정 FIFO/RR에서만 두 계측 선택자를 설정하고 Cargo 기능과 fixed trace/cost는 해제한다. 소스·구성·패키지별로 서로 다른 0이 아닌 십진 u64 이미지 ID를 배정한다. 이를 보관 ELF와 실제 카드 kernel8.img의 SHA-256에 연결하고 소스 리비전·빌드 명령·툴체인을 session.json과 보존한다. ID는 운영자가 부여한 패키지 식별자다. 일반 이미지는 두 변수를 해제하여 빌드하고 ELF와 적재 영역의 해시를 독립 비교한다.

저장소 루트에서 아래를 실행하고 fixed-rr 및 새 ID로 반복한다. 직렬 포트를 열기 전에 패키지를 확인한 카드에 준비한다. PORT·REVISION·EEPROM·CARD_BOOT는 실제 어댑터·보드 리비전·부트로더·탑재한 카드의 부팅 디렉터리다. 수집기는 전원 완전 차단 후 독립 부팅 2회의 원시 바이트와 호스트 읽기 시간을 기록하며, 30초 안에 마지막 행을 받고 1초 동안 추가 바이트가 없는지 확인한다. 최초 미완료도 보존한다. raspi4b에는 동일 이미지로 -M raspi4b -accel tcg -display none -serial stdio -kernel 및 -d int -D를 사용하여 UART와 예외 기록을 보관한다. 해당 모델 실행의 매니페스트로 해독한다. --replay는 직렬 입력 재생이다.

```sh
set -eu
app=fixed-fifo
id=63001
out=$(mktemp -d)
(cd kernel && env -u JNOX_FIXED_TRACE -u JNOX_FIXED_COST JNOX_BOARD=pi4 JNOX_PROFILE=fixed JNOX_APP="$app" JNOX_PI4_WITNESS=1 JNOX_PI4_WITNESS_ID="$id" CARGO_TARGET_DIR="$out/target" cargo build --locked --release -j1)
elf="$out/target/aarch64-unknown-none/release/jnox-kernel"
aarch64-linux-gnu-objcopy -O binary "$elf" "$out/kernel8.img"
python3 tools/package_pi4.py --kernel "$out/kernel8.img" --output "$out/boot"
(cd "$out/boot" && sha256sum -c SHA256SUMS)
aarch64-linux-gnu-nm -S -C "$elf" > "$out/symbols"
aarch64-linux-gnu-objdump -d "$elf" > "$out/disassembly"
python3 tools/pi4_fixed_witness_capture.py --profile "pi4-$app-witness" --image-id "$id" --port "$PORT" --board-revision "$REVISION" --eeprom "$EEPROM" --boot-dir "$CARD_BOOT" --elf "$elf" --output "$out/physical"
python3 tools/pi4_fixed_witness.py "$out/physical/run-1/uart.bin" --profile "pi4-$app-witness" --manifest "$out/physical/session.json" --elf "$elf" --image "$CARD_BOOT/kernel8.img"
python3 tools/test_pi4_fixed_witness.py "$out/physical/run-1/uart.bin" "$elf" "pi4-$app-witness" "$id"
```

PW 스키마 1은 LF로 끝나는 십진 u64 행이다. 세 작업의 실제 종료 뒤 선택된 유휴 스택에서 타이머를 끄고 IRQ를 마스킹한 채 출력한다. 디스패치 64개, 세대당 작업 표본 2개를 예약한다. 0행은 버전·이미지 ID·부팅 카운터·FIFO(0)/RR(1)·MPIDR·CurrentEL·CNTFRQ·디스패치/표본 용량·프레임 크기·커밋 수·두 종류의 손실 수다. 1행은 슬롯·역할·스택 하한/상한·링크된 진입점이다. A/B/HIGH/유휴의 역할은 0/1/2/3, 유휴 슬롯은 4이다. 2행은 세대, 기존 19개 필드(사유, 원시 IAR/ESR, 카운터, 이전/선택 슬롯, 이전 프레임/ELR/SP, 선택 프레임/ELR/SP, 인수, 결과, 비교 활성, 목표, 설정 후 CTL/CVAL, 진입 CTL, 과금), 진입 CVAL·CPU·경계 시점 A/B 진행량·선택 표본이다.

3행은 세대·진입(0)/작업(1)·역할·CPU·프로브 PC/SP·카운터·작업 소유 진입 횟수 또는 루프 진행량이다. 진행 증가 전 읽은 세대를 표본 뒤 다시 확인한다. 이 사이 선점된 표본은 사용하지 않는다. 4행은 최종 세대·CPU·실제 유휴 프로브 PC/SP/시각·종료 진입 PC다. PW END는 총 행 수와 각 태그·단어를 리틀엔디언 8바이트로 누적한 FNV-1a-64이며 마지막 행은 제외한다. SHA-256은 ELF·이미지에 별도 적용한다. 종료 상태 0은 소프트웨어 선점/재개 관측 충족, 2는 유효한 기록이나 행동 미관측, 1은 기록 무효다. 모델의 개별 IRQ/SVC/ERET는 별도 원시 기록과 ELF로 대조한다. 저장 ELR·선택 ELR과 이후 작업 진행으로 복귀 후 작업을 확인하며 실제 ERET 명령 시각·최초 은퇴 명령에는 독립 추적이 필요하다. 관측 SP는 해당 시점 깊이다.

raspi4b 모델의 첫 엔지니어링 부팅 두 건은 종료 자료를 출력했으나 배포 디코더는 모두 종료 코드 1(terminal idle mismatch)로 거절했다. [Pi 4 고정 우선순위 증빙 · 최초 모델 거절](../evidence/pi4-fixed-witness-first.md)

## CHG-068 · v1 디스패치 행의 호스트 해석 수정

#146의 raspi4b FIFO·RR 최초 캡처는 원래 UART·ELF·이미지·매니페스트·예외 로그와 호스트 종료 1(`terminal idle mismatch`)을 보존한다. 태그 2의 페이로드는 세대가 0번이다. 태그를 제외한 0 기준 필드 6–8은 중단된 프레임 주소·ELR·SP이고 9–11은 선택된 프레임 주소·ELR·SP다. 마지막 종료 SVC는 유휴 프레임을 선택하고 ELR을 링크된 종료 루틴으로 설정한다. 태그 4의 5번 필드는 그 루틴의 진입 주소다. 이를 10번과 대조하고 9번에는 유휴 스택 범위·정렬 검사를 유지한다. HIGH 이후 재개 시 선택된 9–11 전체를 중단 시점의 6–8과 비교한다. 기존 해석기는 종료 비교에 9번, 재개 비교에 8–10번을 썼다.

원래 두 캡처를 별도로 식별한 호스트 리비전으로 재생하고 최초 거부와 후속 판정을 함께 보존한다. 입력 해시와 당시 ELF·이미지·매니페스트를 고정하고 대체 부팅을 수집하지 않는다. 선택된 스택 주소와 실행 주소, 다른 유효 실행 주소와 종료 심볼, 변경된 복귀점 및 인과 진행 표본 누락을 구별하는 대조를 요구한다. 기존 세대·위상·작업·프레임·ELF·푸터·진행 검사를 유지한다. 필요한 진행이 관측되지 않으면 부분 결과로 기록한다. 물리 UART 및 독립 하드웨어 실행 추적은 미결이다. [#146 원래 프로토콜과 근거 처리](https://github.com/Romanof123/JNOX/pull/146) · [생성기 리비전](https://github.com/Romanof123/JNOX/tree/c388505e576f3019966ae2c74215767097c6fc18) [최초 모델 실행 근거](../evidence/pi4-fixed-witness-first.md)

[통합 이미지의 첫 부팅과 일반 작업 추적 및 계측 판정](../evidence/pi4-fixed-witness-integration.md)

[RR 순환 후 완료 데이터 인계](pi4-rr-handoff.md)
