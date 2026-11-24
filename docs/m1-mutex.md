# 뮤텍스 경합 시 스케줄링

[English](m1-mutex.en.md) · [변경 이슈 #49](https://github.com/Romanof123/JNOX/issues/49)

CHG-020은 POL-014와 REQ-002/005/023을 DES-003/012, IMP-018, VER-002/005/023에 연결한다. 가상 시험은 Cortex-A72 한 개, GICv2, 물리 범용 타이머 및 특권 작업 문맥 네 개를 쓴다. BASE·LOW·MEDIUM·HIGH의 기본 우선순위는 1·2·5·9이며 큰 수가 우선한다.

| 시험 | 기대하는 결정 |
| --- | --- |
| 직접 상속 | LOW가 X를 가진 상태에서 HIGH와 MEDIUM을 함께 준비한다. HIGH가 X를 요청하면 차단된다. 준비된 MEDIUM보다 상속받은 LOW가 먼저 실행된다. 해제 시 X는 HIGH에 양도되고 LOW는 기본 우선순위를 복구한다. HIGH가 저장된 획득 호출로 복귀해 X를 해제하는지 확인한다. |
| 연쇄 상속과 시간 만료 | BASE가 Y를 보유한다. LOW가 X를 보유한 채 Y를 기다린다. HIGH와 MEDIUM을 준비하고 HIGH가 X를 기다리면 LOW와 BASE 모두 9를 상속받는다. MEDIUM이 준비된 동안 BASE가 실행된다. 물리 타이머 IRQ가 HIGH의 대기를 만료시키면 HIGH는 시간 초과로 복귀한다. HIGH 종료 후 MEDIUM이 앞선 두 소유자보다 먼저 실행된다. |
| 즉시 천장 | X의 천장은 5이며 LOW·MEDIUM이 사용할 수 있다. Y의 천장은 9이며 LOW·HIGH가 사용할 수 있다. LOW가 X와 Y를 차례로 획득하면 HIGH가 기다리기 전부터 5, 9로 올라간다. LOW가 양보한 뒤 HIGH가 Y에서 차단된다. LOW가 내부 Y만 해제하면 HIGH로 소유권이 넘어가고 LOW는 X의 천장 5를 유지한다. 마지막 X를 해제하면 2로 복귀하고 MEDIUM이 실행된다. |

한 작업의 중첩 보유 상한은 두 개이며 동시에 하나의 획득만 기다릴 수 있다. 보유 역순으로 해제한다. 잘못된 사용자 마스크·천장, 권한 없는 획득, 재귀 획득, 대기 순환, 비소유자 해제는 상태 변경 전에 거부한다. 시험에서는 동기화 연산을 직렬화한다. 상속형 뮤텍스에서는 대기자의 현재 유효 우선순위가 소유자에게 전달된다. 즉시 천장형 뮤텍스에서는 획득할 때 소유자를 올린다. 한 구성에서 두 방식을 사용해도 각 규칙의 의미를 유지한다. 양도는 유효 우선순위순이며 동률이면 먼저 요청한 작업이 우선한다.

신규 검증자가 공개 기준으로 첫 실행 결과를 기록한다. 실제 SVC·IRQ 복귀 주소, 작업 문맥, 소유·대기·준비 상태, 변경 전후 우선순위, 타이머 만료와 복원된 스택을 대조한다. 고정 이미지와 기록을 보존한다. 실보드 최대 차단 시간, IRQ 지연, 제품 부하 조건과 기능안전 승인은 별도 추적한다.

## 재현

kernel 디렉터리에서 `cargo build --release --locked --features mutex-bench -j1`로 빌드한다. 저장소 루트에서 `python3 tools/qemu_boot.py --profile mutex --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel`를 실행한다. 부팅 표식 두 줄 이후 MUTEX PI TRANSFER, MUTEX CHAIN TIMEOUT, MUTEX CEILING NESTED, MUTEX PASS를 순서대로 받아야 한다. 5초 안에 수신하고 100 ms 무출력을 확인한 뒤 호스트가 종료한다.

ELF 심볼 mutex_witness의 크기는 1,304바이트다. 리틀엔디언 u64 세 개가 유효 행 수, 타이머 만기, IRQ 시각을 기록한다. 각 32바이트 행은 u64 두 개로 이전 ELR과 문맥 주소를 담고, 이어 1바이트 값 16개로 사례, 동작, 인자, 호출자, 선택 작업, 선택 작업을 포함한 준비 마스크, X·Y 대기 마스크, X·Y 소유자, 작업 번호순 유효 우선순위 네 개, 깨어난 작업 및 인터럽트 ID를 담는다. 255는 해당 값 없음이다. 동작 번호 1~6은 획득·해제·활성화·양보·종료·타이머 만료다. 총 40칸 중 사례별 예상 유효 행은 8·14·12개다. IRQ는 BASE 실행 루프 안에서 발생해야 한다. 같은 실행에서 QMP로 이 표와 예외 기록 및 UART를 확보한 뒤 동결된 ELF의 명령 주소와 대조한다. 새 `MUTEX_CAPTURE_DIR`를 지정하고 `python3 tools/mutex_capture.py "$@"`를 호출하는 Linux 실행 래퍼를 `--qemu` 인자로 준다.

[독립 검증 결과](evidence/evd-018.md)
