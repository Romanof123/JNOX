# 일반 Rust 배치 작업 실행

[English](ordinary-batch.en.md) · 한국어

`JNOX_BATCH_WORKLOAD=telemetry`를 선택하면 같은 정책에서 유한 메모리 입력의 요약·임계값 집계·형식 검증 작업을 실행한다. [텔레메트리 수용 구성과 관측 기준](batch-telemetry-profile.en-ko.md)

단일 코어 AArch64 virt 구성에서 신뢰된 Rust 함수가 `submit(work, argument, estimate, delay)`로 작업을 제출한다. 각 작업은 세대 핸들을 받으며 함수 반환 시 완료된다. 명시적 완료는 현재 실행 세대에만 성공한다. 최대 네 작업에 16 KiB 스택을 각각 할당하고 유휴 루프에 다섯 번째 스택을 배정한다. 추정량의 범위는 카운터 1~4,294,967,295틱이다. 지연 제출도 즉시 슬롯을 예약한다. 승인된 작업은 한 번 실행된다.

| 입력 및 조건 | 결과 |
| --- | --- |
| 추정값 0 또는 2³² | 추정값 오류(1), 세대 유지 |
| 현재 카운터와 지연 합의 오버플로 | 도착 시각 오류(4) |
| 네 슬롯이 모두 사용 중 | 용량 오류(2), 기존 작업 보존 |
| 세대 카운터 소진 | 소진 오류(3), 순환 금지 |
| 다른 작업 또는 종료한 세대의 핸들로 완료 요청 | 만료 오류, 호출자 계속 실행 |
| 실제 서비스량이 추정값 초과 | 완료까지 잔여 순위 1 유지 |
| 대기 순위와 도착 시각 동일 | 먼저 승인한 작업 선택 |
| 실행 작업과 SRTF 순위 동일 | 실행 작업 유지 |

커널은 부팅, 승인·거절 SVC 및 타이머 IRQ 처리 시 CNTPCT를 읽고 직전 선택 이후 구간을 실행 작업에 부과한다. 해당 구간의 커널 오버헤드가 포함된다. CNTP_CVAL은 대기 중 가장 이른 도착을 가리킨다. HRRN은 선언된 만기부터 대기 시간을 계산해 IRQ 처리 지연도 반영한다. 실제 디스패치 이후에 작업이 실행된다. 물리 지연, WCET 및 제품 용량 상한에는 별도의 근거가 필요하다.

## 실행 및 캡처

Linux에서 저장소 루트를 기준으로 실행한다. Rust 1.95.0, aarch64-unknown-none 타깃, Python 3, QEMU AArch64, aarch64-linux-gnu-nm이 필요하다. 새로운 출력 디렉터리에서 순차 실행한다.

```sh
set -eu
out=$(mktemp -d)
unset JNOX_FIXED_TRACE JNOX_BATCH_TRACE
export JNOX_BOARD=virt JNOX_PROFILE=batch
for app in batch-sjf batch-srtf batch-hrrn; do
  export JNOX_APP="$app" CARGO_TARGET_DIR="$out/$app"
  (cd kernel && cargo +1.95.0 build --release --locked -j1)
  elf="$CARGO_TARGET_DIR/aarch64-unknown-none/release/jnox-kernel"
  python3 tools/qemu_boot.py --kernel "$elf" --profile "$app" --runs 2 \
    --timeout 10 --capture-dir "$out/$app-ordinary"
  export JNOX_BATCH_TRACE=1 CARGO_TARGET_DIR="$out/$app-diag"
  (cd kernel && cargo +1.95.0 build --release --locked -j1)
  elf="$CARGO_TARGET_DIR/aarch64-unknown-none/release/jnox-kernel"
  export JNOX_BATCH_CAPTURE_DIR="$out/$app-qmp"
  python3 tools/qemu_boot.py --kernel "$elf" --profile "$app-trace" --runs 1 \
    --timeout 10 --qemu tools/batch_runtime_capture.py
  python3 tools/batch_runtime_observer.py "$app" "$elf" "$JNOX_BATCH_CAPTURE_DIR"
  unset JNOX_BATCH_TRACE
done
```

## 작업과 근거

F는 CNTFRQ(초당 틱)다. 생산자 1의 추정량은 F이고, 먼저 들어온 작업 2·3의 추정량은 2F다. 짧은 작업 4는 추정 F/2, 지연 F/10으로 제출한다. 생산자는 F/10 뒤 F/500의 여유를 둔 시점까지 체크섬을 계산한 후 반환한다. 정수 시간은 내림한다. 후속 작업 5는 F/20 뒤 시작해 한 틱 추정량으로 F/1000만큼 계산한 뒤 한 틱 작업 6을 제출한다. 실제 관측값으로 선택하므로 큰 호스트 또는 IRQ 지연으로 기대 순서가 달라진 실행은 실패 기록으로 보존한다.

| 정책 | 생산자와 동료의 관측 순서 |
| --- | --- |
| SJF | 생산자 종료, 짧은 작업, 선행 1, 선행 2 |
| SRTF | 짧은 작업, 생산자 복원·종료, 선행 1, 선행 2 |
| HRRN | 생산자 종료, 선행 1, 선행 2, 짧은 작업 |

일반 관측기는 전체 작업 출력을 대조하고 100 ms 동안 UART가 그대로인지 확인한 다음 QEMU 종료 뒤 다시 검사한다. 진단 관측기는 최대 64건·24 KiB를 허용하며 각 카운터, 선택, 타이머 비교값, 복원 프레임을 검사한다. Linux QMP는 정숙 구간 뒤 CPU를 정지시켜 실제 유휴 PC를 ELF 심볼과 대조하고 게스트 메모리의 기록 배열을 읽는다. 오프라인 재검증은 UART와 게스트 메모리를 비교하고 소유자별 CPU 누적 틱을 출력한다(유휴 소유자 0).

각 기록의 64비트 필드 15개는 종류(0 부팅, 1 IRQ, 3 제출, 4 완료), CNTPCT, 이탈·진입 세대, 이탈 프레임 주소와 ELR, 진입 프레임 주소와 ELR, 인수 또는 완료 토큰, 추정량, 상대 지연, 상태, 승인 토큰, 선택 작업 잔여량, 하드웨어 비교값(0이면 비활성) 순이다. 완료 상태 1은 만료를 뜻한다. 최초 프레임은 C ABI 진입 함수의 x0~x2와 정렬된 EL1h 스택 상단을 복원한다. SRTF에서 IRQ 이탈 프레임과 ELR을 짧은 작업 완료 뒤 진입 프레임과 대조한다. 별도의 유휴 행에 실제 명령 주소와 SP를 기록한다. 캡처 디렉터리에 UART, 예외, 게스트 메모리 및 정지 시점의 레지스터를 보존한다.

POL-003 / IMP-037. SJF·SRTF·HRRN에서 일반 부팅 6회와 진단 부팅 3회가 통과했으며 진단 기록의 결정 60개를 독립 재현했다. [EVD-037](evidence/evd-037.md)
