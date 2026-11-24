# 일반 Rust 파티션 실행 기록

[English 및 재현 명령](ordinary-partition-capture.en.md) · [동결 계획](pol-006-rust-partition-plan.md) · [#99](https://github.com/Romanof123/JNOX/issues/99) · [#21](https://github.com/Romanof123/JNOX/issues/21)

CHG-046은 REQ-002/005/023에 영향을 준다. IMP-044는 DES-003/012에 따른 해당 요구사항 및 POL-006의 부분 구현이다.  아래 명령을 kernel 디렉터리에서 table=p와 table=q로 각각 실행한다. 각 콜드 부팅의 ELF, 해시, 리비전, 역어셈블리, UART, 예외 로그를 별도 보존한다. 기존 partition-bench 기능은 기존 어셈블리 시험을 실행한다.

virt에서 JNOX_PROFILE와 JNOX_APP를 설정하지 않고 P/Q 하나만 선택한다. 다른 벤치 기능이나 보드와의 조합은 거부한다. 고정된 Rust 툴체인, QEMU, LLVM 도구가 설치된 POSIX 셸을 사용한다. STOP 줄이 완성된 후 QEMU를 종료한다. 시간 초과 또는 끊긴 출력은 미완료로 기록한다.

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

부팅 전 합성 카운터 검사는 지연 시점에 해당하는 선택 결과를 확인한다. 실제 유한 IRQ 마스킹과 실제 1ms 초과 지연 시험, 실측 점유·복구 증거는 미결이다. 실행 결과는 첨부하지 않았다.

## 타이머 비교값 관측 및 오프라인 시간 집계

IMP-044는 복귀 세대마다 TIMER 한 줄을 기록한다. 정상 게스트는 다음 명목 경계 d를 타이머에 쓴다. 카운터 표본 p와 after는 비교값 쓰기와 실제 레지스터 읽기 c를 감싼다. 진입 표본 i는 다음 EDGE 또는 STOP에 있다. 정상 게스트는 c=d를 요구한다. P/Q의 정상 콜드 빌드는 위 명령을 따른다. 게스트 지연 선택자는 제공하지 않는다. #106의 제어된 지연 통지와 #99의 실제 IRQ 마스킹 관측은 미결이다.

분석기는 각 명목 경계에서 구간을 나누어 작업별·유휴·커널·불확실 시간, 실행 자격이 있던 주체의 서비스 손실, 누락 구간, B의 준비 예정 시점과 이후 최초 선택 시점을 보고한다. A의 첫 종료 경계에 대해 c−d, i−c, x−i를 구분한다. 총합은 epoch부터 마지막 진입 표본까지다. JSON의 origin은 호출자가 지정한 입력 종류다. 게스트 판정은 원본 예외 복귀 기록과 ELF의 독립 대조가 필요하다. 아래 합성 6개 조합은 호스트 산술 검사용이다.

```sh
# From repository root, after collecting a fresh healthy guest UART:
python tools/ordinary_partition_account.py capture-ordinary-p/uart --table P --case healthy --origin guest > guest-P.json
python tools/ordinary_partition_account.py capture-ordinary-q/uart --table Q --case healthy --origin guest > guest-Q.json
# Offline synthetic controls, with no guest:
for table in P Q; do
  for mode in healthy near long; do
    python tools/test_ordinary_partition_account.py --emit "$table" "$mode" > "synthetic-$table-$mode.uart" || exit 1
    python tools/ordinary_partition_account.py "synthetic-$table-$mode.uart" --table "$table" --case "$mode" --origin synthetic > "synthetic-$table-$mode.json" || exit 1
  done
done
python tools/test_ordinary_partition_account.py
```

POL-006 / IMP-044. 최초 일반 실행 두 건 모두 1 ms 기준에 실패했고 Q는 준비된 B 창 하나를 건너뛰었다. 실제 마스킹 및 전체 릴리스 적격성은 미확정이다. [EVD-044](evidence/evd-044.md)
