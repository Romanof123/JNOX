# AMC sensor clock-pair cohorts / AMC 센서 시계 쌍

The four slot wrappers select the paired sampler in the original ring worker. The original sources in ../native-amc continue to select their original two clock reads. The ring, 256-sample batch, 256 batches per clock observation, 8 ms CPU stamp spacing, generation inputs and 936-byte amc_product stay the same. Use Bash from the repository root.

슬롯별 래퍼는 기존 링 작업의 시계 표본 함수만 선택한다. 기존 입력·배치·표본 주기와 amc_product 형식을 보존한다. 저장소 루트의 Bash에서 실행한다.

~~~bash
out=output/amc-clock
src=examples/native-amc-clock
mkdir -p "$out"
for i in 0 1 2 3; do
    sh sdk/native-aarch64/build.sh "$src/slot$i.c" "$out/slot$i.elf"
done
pack=(python3 tools/build_native_el0.py --policy amc)
four=(--elf "$out/slot0.elf" --peer-elf "$out/slot1.elf" --peer-elf "$out/slot2.elf" --peer-elf "$out/slot3.elf")
for name in normal-hi staggered; do
    "${pack[@]}" "${four[@]}" --out "$out/page-$name" --amc-clock-page \
        --amc-manifest "examples/native-amc/config/$name.json"
done
"${pack[@]}" "${four[@]}" --out "$out/fallback-normal-hi" --amc-manifest examples/native-amc/config/normal-hi.json

sh sdk/native-aarch64/build.sh "$src/control0.c" "$out/low-control.elf"
sh sdk/native-aarch64/build-rust.sh "$src/control-rust/Cargo.toml" "$out/high-control.elf"
"${pack[@]}" --out "$out/brackets" --amc-clock-page --amc-manifest "$src/control.json" \
    --elf "$out/low-control.elf" --peer-elf "$out/high-control.elf"
~~~

Keep the same four ELF hashes for enabled and disabled positions. Record the kernel, manifest, SDK/toolchain identity, ELF disassembly and actual backend before interpreting timing. Order: enabled normal-hi plain/traced, enabled staggered plain/traced, disabled normal-hi traced, control plain. Both control ELFs explicitly check CPU and monotonic syscall brackets around the helper. In high-control the C reducer calls the public Rust clock_sample through rust_clock_sample; low-control uses the C helper directly. Low streams 70 CPU ms in generation zero and sums eight readings in generation one, returning 0. High streams 20 CPU ms and returns 1.

활성·비활성 패키지는 네 앱 해시를 동일하게 유지한다. 대조군은 두 시계 각각에 대해 도우미 값이 전후 실제 호출 6 표본 범위에 있는지 확인한다. 첫 저우선순위 세대 중 선점과 두 번째 세대의 재초기화는 실제 복귀 기록·이전 세대 RAM·새 결과를 대조한다.

Read amc_clock_evidence separately from amc_product in each generation’s private mapping or retained retirement snapshot. All integers are little-endian. At offsets 0, 8, 16 are u64 counts of failed, page-backed and syscall-backed pairs. At offsets 24 and 28 are the u32 committed bracket count and overflow flag. The timed quartet exports just this 32-byte header; each compatibility ELF also exports 1024 rows beginning at offset 32. Each 72-byte row contains nine u64 values: generation, completed sample count, CPU before, monotonic before, paired CPU, paired monotonic, monotonic after, CPU after, backend. Clock values are nanoseconds. Compare corresponding clocks inclusively and report both widths. An overflow marks an incomplete table. For the disabled run correlate actual CPU-then-monotonic SVCs and backend 2. For enabled timed positions correlate backend 1 with the linked page-reader path and the actual entry cause; fallback counts remain separately visible.

시계 증거는 세대별 amc_clock_evidence, 원래 링 결과는 amc_product에서 읽는다. 백엔드 집계와 실제 시계 호출·물리 IRQ를 대응시킨다. 활성화 플래그만으로 예산 전환을 일으킨 예외를 판정할 수 없으므로 해당 실행의 벡터 기록을 확인한다. [Frozen positions / 고정 판정 위치](../../docs/native-amc-clock-pair.en-ko.md)
