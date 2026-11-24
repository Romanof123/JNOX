# 일반 Rust 파티션 실행 기록

[English 및 재현 명령](ordinary-partition-capture.en.md) · [동결 계획](pol-006-rust-partition-plan.md) · [#99](https://github.com/Romanof123/JNOX/issues/99) · [#21](https://github.com/Romanof123/JNOX/issues/21)

CHG-046은 REQ-002/005/023에 영향을 준다. IMP-044는 DES-003/012에 따른 해당 요구사항 및 POL-006의 부분 구현이다.  아래 명령을 kernel 디렉터리에서 table=p와 table=q로 각각 실행한다. 각 콜드 부팅의 ELF, 해시, 리비전, 역어셈블리, UART, 예외 로그를 별도 보존한다. 기존 partition-bench 기능은 기존 어셈블리 시험을 실행한다.

virt에서 JNOX_PROFILE와 JNOX_APP를 설정하지 않으면 기본 빌드는 부팅 이미지를 만든다. `ordinary-partition-p` 또는 `ordinary-partition-q` 단독 선택은 해당 표를 실행한다. 기존 벤치 없이 두 표를 함께 선택하면 모호하므로 거부한다. 일반 파티션 표에는 virt가 필요하다. 기존 벤치 기능과 조합하면 기존 우선순위가 진입점을 결정한다. `partition-bench,ordinary-partition-q`는 어셈블리 시험, `cbs-bench,ordinary-partition-p`는 CBS, `--all-features`는 `coop-bench`와 동일한 협력형 진입점을 선택한다. 기존 보드·프로필 제약은 계속 적용된다. 저장소 루트에서 `python tools/test_partition_selectors.py`로 이 선택 규칙을 검사한다. 고정된 Rust 툴체인, QEMU, LLVM 도구가 설치된 POSIX 셸을 사용한다. STOP 줄이 완성된 후 QEMU를 종료한다. 시간 초과 또는 끊긴 출력은 미완료로 기록한다.

```sh
table=p # repeat with q
out="../capture-ordinary-$table"
mkdir "$out" || exit 1
cargo build --release --features "ordinary-partition-$table" || exit 1
cp target/aarch64-unknown-none/release/jnox-kernel "$out/kernel.elf"
git rev-parse HEAD > "$out/revision"
git write-tree >> "$out/revision"
sha256sum "$out/kernel.elf" > "$out/sha256"
llvm-objdump -d "$out/kernel.elf" > "$out/disassembly"
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 \
  -accel tcg -m 128M -nodefaults -display none -monitor none -serial stdio \
  -d int -D "$out/exceptions.log" -kernel "$out/kernel.elf" | tee "$out/uart"
```

모든 수치는 십진수 물리 카운터 틱이다. 소유자 0/1/2는 A/B/유휴이며 3은 부팅 표시다. STACK의 상한은 제외한다. 진입 표본은 벡터 분기와 스택 저장 뒤에 기록한다. 진단 한도인 네 프레임을 벗어나면 기록을 기각한다.

A와 B는 독립 스택에서 서로 다른 누산기를 갱신한다. B는 첫 명시적 유휴 구간에서 준비된다. 유휴 컨텍스트도 독립 스택을 사용한다. HEADER의 epoch와 주파수로 명목 경계를 복원한다. EDGE의 due, entry, live, ret, next는 각각 직전 비교값, IRQ 진입 표본, 선택 시점, 예외 복귀 직전 표본, 다음 비교값이다. OBS는 복귀 후 실제 태스크가 기록한 카운터와 진행량이다. 같은 소유자의 프레임 경계도 개별 관측한다. STACK, 역어셈블리, 독립 예외 복귀 기록과 SP·PC·레지스터 지문을 대조한다. BOUND의 skipped=1은 선택 시점에 이미 끝난 구간이다. STOP은 마지막 태스크의 종료 표본을 제공한다.

OBS부터 마지막 태스크 표본까지의 실행은 해당 태스크에, 진입부터 복귀 직전까지는 커널에 귀속한다. 그 사이의 불확실 구간은 명목 경계별로 분할하고 유휴 시간을 따로 계산한다. 시작 시각부터 최초 복귀 직전까지도 포함한다. 전체 지연 상한은 F/1000이다. 판정에는 세 프레임과 다음 프레임의 최초 복귀 증거가 필요하다. 유실, 불명확한 귀속, 누락 구간, 상한 초과는 계획의 실패·미확정 조건을 적용한다.

부팅 전 합성 카운터 검사는 지연 시점에 해당하는 선택 결과를 확인한다. 실제 유한 IRQ 마스킹과 실제 1ms 초과 지연 시험, 실측 점유·복구 증거는 미결이다.

## 타이머 비교값 관측 및 오프라인 시간 집계

IMP-044는 복귀 세대마다 TIMER 한 줄을 기록한다. 정상 게스트는 다음 명목 경계 d를 타이머에 쓴다. 카운터 표본 p와 after는 비교값 쓰기와 실제 레지스터 읽기 c를 감싼다. 진입 표본 i는 다음 EDGE 또는 STOP에 있다. 정상 게스트는 c=d를 요구한다. 위 명령은 `JNOX_PARTITION_NOTICE`를 설정하지 않은 정상 P/Q 실행에 사용한다.

분석기는 각 명목 경계에서 구간을 나누어 작업별·유휴·커널·불확실 시간, 실행 자격이 있던 주체의 서비스 손실, 누락 구간, B의 준비 예정 시점과 이후 최초 선택 시점을 보고한다. A의 첫 종료 경계에 대해 c−d, i−c, x−i를 구분한다. 총합은 epoch부터 마지막 진입 표본까지다. JSON의 origin은 호출자가 지정한 입력 종류다. 게스트 판정은 원본 예외 복귀 기록과 ELF의 독립 대조가 필요하다. 아래 합성 6개 조합은 호스트 산술 검사용이다.

```sh
# From repository root, after collecting a fresh healthy guest UART:
python tools/ordinary_partition_account.py capture-ordinary-p/uart --table P --case healthy --origin guest --elf capture-ordinary-p/kernel.elf --elf-sha256 "$(cut -d' ' -f1 capture-ordinary-p/sha256)" --exceptions capture-ordinary-p/exceptions.log > guest-P.json
python tools/ordinary_partition_account.py capture-ordinary-q/uart --table Q --case healthy --origin guest --elf capture-ordinary-q/kernel.elf --elf-sha256 "$(cut -d' ' -f1 capture-ordinary-q/sha256)" --exceptions capture-ordinary-q/exceptions.log > guest-Q.json
# Offline synthetic controls, with no guest:
for table in P Q; do
  for mode in healthy near long; do
    python tools/test_ordinary_partition_account.py --emit "$table" "$mode" > "synthetic-$table-$mode.uart" || exit 1
    python tools/ordinary_partition_account.py "synthetic-$table-$mode.uart" --table "$table" --case "$mode" --origin synthetic > "synthetic-$table-$mode.json" || exit 1
  done
done
python tools/test_ordinary_partition_account.py
```

게스트 입력에는 원본 ELF, 독립 부팅 기록의 SHA-256 및 QEMU 예외 로그가 필요하다. llvm-nm 또는 aarch64-linux-gnu-nm로 ELF를 읽고 최초 PC·SP, 중단 PC의 작업 심볼 범위, 전용 스택, 순서대로 기록된 예외 복귀와 IRQ ELR을 대조한다. 심볼 형식을 알 수 없거나 증거가 누락되면 판정할 수 없다. 누산기는 작업별 초기값부터 관측된 반복 횟수까지 독립적으로 재계산하며 전체 상한은 6,400만 회다. 합성 입력은 고정된 가상 심볼 배치를 사용하며 ELF·예외 로그 검증 대상에서 제외한다.

b_release_nominal은 B가 준비되는 명목 시점, b_first_selection은 B를 실제 선택한 디스패처 표본, b_first_worker_observation은 그 뒤 B가 직접 남긴 최초 표본이다. 모두 원시 카운터 틱 단위다. 정상 게스트의 완전한 기록이 지연 기준을 위반하면 JSON에 초과 경계와 누락 슬롯을 남기고 상태 2로 종료한다. 상태 1은 손상·미완료 입력이다. 이 변경은 최초 P/Q 관측을 보존하며 실제 지연을 개선하지 않는다.

위 명령으로 현재 합성 입력을 생성한다. v1에 보존된 이전 형식 입력 6개는 현 판정기가 거절하는 사례다.

POL-006 / IMP-044. v2의 새 정상 콜드 실행은 P 10/10, Q 13/13 경계에서 1 ms를 충족했다. v1은 P 9/10, Q 11/12 경계가 초과했고 Q가 준비된 B 구간을 건너뛰었다. 실제 지연 주입과 IRQ 마스킹은 미검증이다. [EVD-044 v2](evidence/evd-044-v2.md)

아래 제어 사례 변경은 독립 검증을 기다리고 있다.

POL-006 / IMP-044. 최초 일반 실행 두 건 모두 1 ms 기준에 실패했고 Q는 준비된 B 창 하나를 건너뛰었다. 실제 마스킹 및 전체 릴리스 적격성은 미확정이다. [EVD-044](evidence/evd-044.md)

## virt의 제어된 타이머 통지

`JNOX_PARTITION_NOTICE=short` 또는 `long`과 실제로 선택된 일반 P/Q 표 하나를 조합하면 해당 콜드 부팅 사례가 실행된다. 부팅 시 NOTICE와 ELF 진입 심볼이 같은 사례를 나타낸다. A의 최초 창에만 short는 c=d+F/1000+1, long은 c=d+5q/4를 쓴다. EDGE의 due/next와 TIMER의 d는 원래 일정을 유지한다. 두 작업은 각자의 스택에서 IRQ를 허용한 기존 루프를 실행한다.

저장소 루트의 Linux 셸에서 아래 명령으로 네 사례를 한 번씩 빌드하고 수집한다. 고정 Rust 툴체인, QEMU, GNU timeout 및 LLVM이나 AArch64 GNU의 objdump와 nm가 필요하다. 별도로 빌드된 ELF를 검증할 때는 제작자가 전달한 SHA-256과 새 출력 경로로 수집 스크립트를 직접 실행한다. 빌드 커밋을 해시와 함께 보관한다. checker-revision 파일에는 수집기 측 체크아웃을 기록한다.

```sh
set -eu
unset JNOX_PROFILE JNOX_APP
out=$(mktemp -d)
printf '%s\n' "$out"
for table in P Q; do
  feature=$(printf '%s' "$table" | tr 'PQ' 'pq')
  for mode in short long; do
    build="$out/build-$table-$mode"
    (cd kernel && JNOX_BOARD=virt JNOX_PARTITION_NOTICE="$mode" \
      cargo build --release --locked -j 1 --target-dir "$build" --features "ordinary-partition-$feature")
    elf="$build/aarch64-unknown-none/release/jnox-kernel"
    sha=$(sha256sum "$elf" | cut -d' ' -f1)
    status=0
    sh tools/capture_partition_notice.sh "$table" "$mode" "$elf" "$sha" "$out/$table-$mode" || status=$?
    printf '%s %s %s\n' "$table" "$mode" "$status"
  done
done
```

호스트는 8초 뒤 게스트를 종료하고 UART, QEMU 진단, 예외 기록, ELF, 역어셈블리와 첫 상태를 보존한다. QEMU 출력 파일은 각각 1,024블록으로 제한한다. 완전한 STOP 줄과 독립 예외 로그의 마지막 IRQ가 필요하다. 오프라인 CLI는 `--case short`와 기존 이름 `near`를 모두 받는다. `--origin guest`에는 수집한 ELF, SHA-256 및 exceptions.log를 함께 지정한다.

RELEASE의 due는 B의 준비 예정 시점이고, before와 after는 n번째 세대에서 실제로 B를 준비 상태로 변경한 시각의 범위다. 각 IRQ 줄은 타이머를 끄기 전의 승인된 인터럽트, 현재 비교값, 제어 레지스터, 중단된 문맥의 SPSR과 IRQ 진입 시각으로 계산한 슬롯 종료 틱을 담는다. 판정기는 마지막 IRQ까지 EL1 벡터 주소와 복귀 PC를 ELF에 대조한다. 복구 대상 창에 들어간 뒤 A가 직접 기록한 서로 다른 반복 표본 두 개를 요구한다. A의 실행 시간은 마지막 작업 표본까지 원래 경계마다 나눠 귀속하고, 그 뒤 IRQ 진입 표본까지는 전환 불확실 시간으로 집계한다.

결과에는 각 창에서 서비스를 잃은 주체, 잔여 시간에 실제로 복귀한 작업과 관측된 작업 시간, 다음 두 보호 창과 원래 세 프레임 구간이 남는다. P long에서는 차단된 B 창 전체와 뒤이은 명시적 유휴 창 일부의 유휴 서비스를 잃는다. Q long에서는 프레임 경계 전후의 서로 다른 두 B 창에서 준비된 B의 서비스를 잃는다. short에는 완전히 누락된 창이 없다. 지정한 경계에서 기존 시간 상한 위반을 실측하고 정해진 복구를 확인하며 나머지 경계가 모두 상한을 만족해야 사례가 통과한다. 상태 2는 다른 경계의 상한 위반이 있는 복원 결과, 상태 1은 잘못되거나 불완전한 증거를 나타낸다. 기존 정상·합성 입력 형식은 그대로 쓴다. 작업 자체의 실제 IRQ 마스킹 및 물리 플랫폼 적격성은 별도 미결 항목이다. [제어 사례 계획](https://github.com/Romanof123/JNOX/pull/106)
