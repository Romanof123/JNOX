# 실행량과 시간을 제한하는 위임 스케줄링 확장

[English](m4-bounded-extension.en.md) · [변경 이슈 #59](https://github.com/Romanof123/JNOX/issues/59)

CHG-028 · POL-018 · IMP-026 · 예정 증적 EVD-026. 선택형 M4 시제품에서 데이터로 작성한 프로그램은 다음에 실행할 위임 스레드를 제안한다. 가상 Cortex-A72 한 코어에서 커널은 평가 시간을 제한하고, 제안을 검증하며, 필요하면 대체 작업을 고른다. 이 과정에서도 독립 작업에 미리 배정한 세 구간을 보장하는지 시험한다. 관련 요구사항은 REQ-023이며 설계와 검증 항목은 DES-012, VER-023이다.

## 결정과 시간 한계

인터프리터는 `IfReady`, `Jump`, `Propose`, `Yield`를 지원한다. 작업 레코드는 최대 32개, 프로그램 명령은 최대 16개다. 뒤로 가는 분기와 종료 명령을 포함해 명령 하나를 가져올 때마다 실행 예산 한 단위를 차감한다. 인터페이스가 허용하는 예산은 최대 16단위이고 이 게스트는 5단위를 준다. 커널은 제안된 작업 ID·세대, 준비 상태, 작업에 허용된 CPU, 현재 사용 가능한 CPU 및 요청 길이를 검사한다. 허용량은 현재 예산과 다음 예약까지의 시간을 모두 충족해야 한다. 제안을 사용할 수 없으면 실행 가능한 위임 작업 중 ID가 가장 작은 작업을 그 작업에 허용된 최소 번호 CPU에 한 틱 배정한다. 실행 가능한 작업이 없으면 배정량을 0으로 두고 전용 유휴 문맥을 실행한다.

EL1 물리 카운터의 주파수를 `F`라 하면 한 슬롯은 `Q = floor(F/10)` 카운터 틱이고, 슬롯 `i`의 시작은 `S = origin + i*Q`다. 평가기는 IRQ를 허용한 상태로 전용 스택에서 정책 복사본을 실행한다. 절대 타이머 만기는 `S + floor(Q/4)`다. 평가기는 결과와 진행 카운터를 게시하기 직전에 잠시 IRQ를 막고 시각을 검사한다. 만기에는 커널이 완성된 결과를 반영한다. 결과가 없으면 보존된 정책 상태와 관측된 진행 카운터로 대체 결정을 내린다. 선택된 문맥은 해당 슬롯에서 처음 실행된 시각을 직접 기록하며, 이 값은 `S + floor(Q/2)`보다 작아야 한다. 배정 구간의 종료 비교값은 `S + Q`다. 게스트 시계 기준으로 각각 약 25, 50, 100 ms다. 평가와 예외 처리에 걸린 시간도 위임 슬롯 안에서 소모된다.

## 9개 슬롯의 시험 부하

작업 0·1·2의 세대 번호는 각각 7·11·17이다. 사용할 수 있는 CPU는 0번이다. 작업 0은 세 슬롯을 주기로 오프셋 2에서 한 슬롯을 예약한다. 커널은 이 구간에서 평가기를 거치지 않고 예약 작업만 선택한다. 슬롯 0부터 4까지 관측하는 진행 카운터는 1, 2, 2, 3, 4이며 이후에는 4를 유지한다. 마지막 증가로부터 두 슬롯 이상 지나면 감시 타이머가 정지 상태를 판정한다.

| 슬롯 | 입력 | 결정과 실제 실행 문맥 | 실행한 명령 수 |
|---:|---|---|---:|
| 0 | 준비 상태 검사 후 작업 2를 정상 제안 | 승인, 제안된 작업 2 실행 | 2 |
| 1 | 작업 2가 CPU 1 요청 | 거부, 대체 작업 1 실행 | 1 |
| 2 | 독립 예약 | 예약 작업 0 실행 | 0 |
| 3 | 작업 2가 이전 세대 16 제시 | 거부, 대체 작업 1 실행 | 1 |
| 4 | 0번 명령으로 되돌아가는 분기 반복 | 명령 예산 소진, 대체 작업 1 실행 | 5 |
| 5 | 독립 예약 | 예약 작업 0 실행 | 0 |
| 6 | 생산자가 새 진행 값을 게시하지 않고 반복 | 감시 시간 초과, 대체 작업 1 실행 | 0 |
| 7 | 생산자 정지 지속, 두 위임 작업 모두 준비되지 않음 | 감시 시간 초과, 유휴 문맥 3에서 WFI 실행 | 0 |
| 8 | 독립 예약 | 예약 작업 0 실행 | 0 |

## 재실행과 기록 확인

Linux의 저장소 루트에서 AArch64 ELF를 빌드하고 부팅 한 번을 캡처한다. QEMU와 AArch64 GNU 도구를 사용한다. 재실행할 때마다 새 캡처 디렉터리를 쓴다.

```sh
(cd kernel && cargo build --release --locked --features extension-bench -j1)
elf=kernel/target/aarch64-unknown-none/release/jnox-kernel
run_dir=$(mktemp -d)
export EXTENSION_CAPTURE_DIR="$run_dir/capture"
cat > "$run_dir/qemu" <<'SH'
#!/bin/sh
exec python3 -B tools/extension_capture.py "$@"
SH
chmod +x "$run_dir/qemu"
python3 tools/qemu_boot.py --profile extension --kernel "$elf" --qemu "$run_dir/qemu"
aarch64-linux-gnu-nm -n -S "$elf" > "$run_dir/symbols.txt"
aarch64-linux-gnu-objdump -d "$elf" > "$run_dir/disassembly.txt"
sha256sum "$elf" "$EXTENSION_CAPTURE_DIR"/* > "$run_dir/SHA256SUMS"
```

같은 부팅에 사용한 소스 리비전과 ELF, 캡처 산출물을 함께 보존한다. 래퍼는 UART, QEMU 인터럽트 추적, 시작·종료 기록, QMP의 CPU·실행 상태를 수집하고 물리 메모리 스냅숏을 `audit.bin`에 저장한다. 정상 UART의 마지막 세 줄은 `EXTENSION EMPTY IDLE`, `EXTENSION RESERVED THREE`, `EXTENSION PASS`다. 게스트 내부 조건을 위반하면 `EXTENSION FAIL`을 출력하고 대기한다. 성공 표식이 누락되거나 저널이 관측과 다르면 재실행 검증 실패로 처리한다.

### 저널 ABI

`extension_witness` 크기는 1,856바이트이며 각 필드는 리틀엔디언 u64다. 헤더 일곱 필드는 순서대로 `count, frequency, origin, slot_ticks, eval_ticks, fallback_ticks, watchdog`이다. 완료 후 `count`는 9다. 각 행은 200바이트이며, 다음 25개 필드가 순서대로 들어간다.

```text
slot ready_mask proposed_id proposed_generation proposed_cpu
reason steps selected grant progress
eval_cval eval_irq eval_pc eval_frame eval_work
decision_time dispatch_pc first_work service_cval service_irq
service_pc service_frame service_work eval_ppi service_ppi
```

이 시험의 사유 코드는 0 승인, 1 거부, 2 명령 예산 소진, 3 진행 정지, 7 예약이다. 제안 ID·제안 CPU·선택 작업·PPI에 해당 값이 없으면 255를 쓴다. 세대 번호가 없으면 0이고 평가기를 실행하지 않은 행의 평가 시각·PC·프레임·작업량도 0이다. 예약 슬롯에는 평가기 IRQ가 없다. 유휴 슬롯은 `selected=255`, `grant=0`과 함께 실제 유휴 프레임 및 PC를 남긴다. 나머지 슬롯의 배정량은 한 틱이다.

독립 재검증에서는 표의 아홉 결정과 `int.trace`의 평가기 만기 6회, 서비스 만기 9회를 대조한다. 매 IRQ 진입에서 물리 타이머 PPI 30이 활성화·마스크 해제·만기 상태여야 한다. 비교 레지스터 값은 절대 만기와 같아야 한다. 위임 슬롯에서는 `eval_cval <= eval_irq <= decision_time <= first_work < S + fallback_ticks`를, 모든 슬롯에서 `service_cval <= service_irq < service_cval + eval_ticks`를 확인한다. 같은 ELF의 심볼과 디스어셈블리로 디스패치 주소와 이후 IRQ가 저장한 `service_pc`를 해당 문맥에 대응시킨다. 서비스 문맥이 다시 선택되면 이전에 저장한 PC와 전용 프레임에서 복귀하고 작업 카운터가 증가한다. 정지한 평가기의 루프 진행 값은 0이다.

정책과 커널 검증 코드, 타이머·저널 처리, 어셈블리 작업 루프 및 호스트 결정 시험을 함께 확인할 수 있다. [인터프리터](../kernel/src/extension_policy.rs) · [제안 검증](../kernel/src/group_policy.rs) · [게스트](../kernel/src/extension.rs) · [작업 코드](../kernel/src/extension.S) · [호스트 시험](../tools/extension_tests.rs)

인터프리터와 커널 검증 코드는 EL1의 같은 주소 공간에서 실행된다. 이 시제품은 전용 스택, 입력 검증, 명령 수 및 시간 제한에 의한 보호를 시험한다. 제품 채택, 임의 네이티브 코드에 대한 격리, 실물 하드웨어 최악 응답시간, 제품 부하 적격성, 위험 분석 및 기능안전 승인은 별도 과제로 남는다.

독립 가상 게스트 결과와 시험 이미지 식별자는 다음 기록에서 확인할 수 있다. [EVD-026](evidence/evd-026.md)
