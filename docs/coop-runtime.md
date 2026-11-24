# 링크형 협력 이벤트 서비스

[English](coop-runtime.en.md) · [EVD-036](evidence/evd-036.md)

CHG-038 / IMP-036 / EVD-036, [#80](https://github.com/Romanof123/JNOX/issues/80) / #21 POL-001. virt 단일 코어의 신뢰된 Rust 태스크는 기존 고정 우선순위 FIFO 커널에서 동일 우선순위로 실행한다. 소유자는 고정 용량 큐와 핸들러 표를 구성한다. 메시지는 핸들러 번호와 u64 값이다. 잘못된 번호와 용량 초과는 각각 InvalidHandler, Full과 원본 메시지를 반환한다. 한 턴은 가장 오래된 메시지의 슬롯을 먼저 비운 후 등록 함수를 한 번 호출한다. 함수에는 추가 전용 콜백을 주어 재귀 디스패치를 막고 기존 메시지 뒤에 추가한다. 성공한 추가는 최대 깊이를 갱신한다. 물리 카운터로 인터럽트를 포함한 경과 틱을 측정하여 핸들러별 완료 횟수, 누적 틱, 최대 틱, 예산 초과 횟수를 기록한다. 누적값은 포화시킨다. 반환 직후 소유자가 초과 여부를 받아 대응한다. 실행 시간 상한 보장은 별도 검증 대상이다.

coop-events에서 소유자(슬롯 1)가 실행 중일 때 대기 중인 슬롯 2 뒤로 타이머가 슬롯 0을 깨운다. 타이머 처리 후 자발적 양보까지 소유자가 유지되고 2, 0 순으로 종료한 뒤 소유자가 복귀해야 한다. 실제 IRQ, 양보 전 타이머 해제, UART 순서와 종단 idle PC를 검사한다. 관측 심볼 coop_app_observations의 리틀 엔디언 u64 순서는 ABI 버전, 용량, 등록 수, 최대 깊이, 가득 참 거절 수, 무효 번호 거절 수, 처리 수, 페이로드 네 칸, 소유자 대응 수, 양보 전 타이머 해제, 십진수로 기록한 동료 순서, 핸들러별 카운터 네 개씩 두 행이다. 미사용 행은 0이다. 용량 2, 핸들러 2에서 11/22/33 처리, 최대 깊이 2, 각 거절 1회, 핸들러 1의 예산(1틱) 초과 대응 2회, 핸들러 0 완료 1회를 확인한다. 11 처리 중 33을 추가한다. coop-single은 용량 및 핸들러 수가 1이고 11/33을 처리하며 최대 깊이 1, 각 거절 1회, 대응 2회를 확인한다. 소유자는 초과를 기록한 뒤 계속 처리한다. 측정 시간의 양수 여부 및 누적/최대/횟수 관계를 검증하며 각 ELF 식별자, UART, IRQ, 관측 메모리 및 idle PC를 보관한다. 반환하지 않는 함수에는 완료 측정값이 없다.

ABI 마지막 네 칸은 호출 순서대로 각 완료의 경과 틱을 저장하며 남은 칸은 0이다. 이 값과 등록 예산으로 핸들러별 통계를 재계산한다. IRQ 직전 PC 복원과 SVC 즉시값 0/1/1/1을 검사한다.

Linux 저장소 루트에서 고정 Rust 도구 체인, AArch64 binutils와 QEMU로 실행한다.
```sh
out=$(mktemp -d)
rustc --edition=2024 --test tools/coop_service_tests.rs -o "$out/host-tests" && "$out/host-tests"
for app in coop-events coop-single; do
  (cd kernel && JNOX_BOARD=virt JNOX_PROFILE=fixed JNOX_APP="$app" CARGO_TARGET_DIR="$out/$app" cargo build --locked --release) || exit 1
  elf="$out/$app/aarch64-unknown-none/release/jnox-kernel"
  COOP_CAPTURE_DIR="$out/$app-raw" python3 tools/qemu_boot.py --kernel "$elf" --profile "$app" --qemu tools/coop_runtime_capture.py || exit 1
  python3 tools/coop_runtime_check.py "$app" "$out/$app-raw" || exit 1
done
```
