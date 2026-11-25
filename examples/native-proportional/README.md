# Proportional native workloads / 네이티브 비례 배분 작업

Build the ELFs once from the repository root under Bash on Linux or WSL. The three stable packages share identical input bytes and slot order. Linux 또는 WSL의 Bash에서 저장소 루트를 기준으로 실행한다. 안정 부하 세 구성은 같은 바이너리와 슬롯 순서를 사용한다.

~~~bash
out=output/native-proportional
mkdir -p "$out"
sh sdk/native-aarch64/build.sh examples/native-weighted-analytics/checksum.c "$out/crc.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-weighted-analytics-rust/Cargo.toml "$out/histogram.elf"
sh sdk/native-aarch64/build.sh examples/native-weighted-analytics/energy.c "$out/energy.elf"
sh sdk/native-aarch64/build.sh examples/native-proportional/reporter.c "$out/report.elf"
pack=(python3 tools/build_native_el0.py --quantum-ticks 625000)
three=(--elf "$out/crc.elf" --weight 1 --peer-elf "$out/histogram.elf" --peer-weight 2 --peer-elf "$out/energy.elf" --peer-weight 4)
pair=(--elf "$out/report.elf" --weight 1 --peer-elf "$out/crc.elf" --peer-weight 2)
"${pack[@]}" --out "$out/lottery1" --policy lottery --lottery-seed 1 --lottery-window 48,24 --lottery-floors 5,3,11 "${three[@]}"
"${pack[@]}" --out "$out/lottery2" --policy lottery --lottery-seed 2 --lottery-window 48,24 --lottery-floors 1,7,10 "${three[@]}"
"${pack[@]}" --out "$out/stride" --policy stride "${three[@]}"
"${pack[@]}" --out "$out/dynamic-lottery" --policy lottery --lottery-seed 1 "${pair[@]}"
"${pack[@]}" --out "$out/dynamic-stride" --policy stride "${pair[@]}"
"${pack[@]}" --out "$out/solo" --policy lottery --lottery-seed 1 --elf "$out/crc.elf" --weight 1
~~~

The separate refusal below requests six grants for A in every 24-selection window. With seed 1, A gets five in [24,48). 이 거부 대조는 모든 24회 창에서 A에 6회를 요구한다. 시드 1의 [24,48)에서 A는 5회여서 패키지가 거부되어야 한다.

~~~bash
"${pack[@]}" --out "$out/refused" --policy lottery --lottery-seed 1 --lottery-window 48,24 --lottery-floors 6,3,11 "${three[@]}"
~~~

Boot each accepted image with the default virtual clock. Use a fresh output path for its external exception trace (-d int -D logfile). 승인된 이미지는 기본 가상 시계로 부팅하고 예외 추적은 실행마다 새 경로에 보존한다.

~~~bash
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
  -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel "$out/lottery1/kernel.elf"
python3 examples/native-weighted-analytics/oracle.py
~~~

| ELF | Final progress / 최종 진행 | Full private result / 전체 결과 | Exit / 종료 |
| --- | ---: | --- | ---: |
| CRC | 134217728 bytes | 2124873618 | 237 |
| Histogram | 536870912 bytes | 2097152 × [1,8,28,56,70,56,28,8,1] | 70 |
| Energy | 1073741824 samples | 5864240971776 | 179 |
| Dynamic reporter | 3 blocks, 12288 bytes | 256 per round, cumulative [256,512,768] | 53 |

The Python oracle covers the three stable workers. For the dynamic reporter, each 4096-byte ramp contains sixteen repetitions of every byte, giving 256 samples ≥240. Check round_count, cumulative_count, private_blocks, private_bytes and private_result. Each round commits the reduction (round_stage=1), yields and returns (2), then requests an absolute wait at a fresh SDK clock sample +40 ms. Require observed_ns[r] ≥ request_ns[r] after each return (3). Use the kernel records to identify shorter service intervals, actual wakes and background progress. For the solo CRC check the same full product and actual quantum-driven selections.

Python 검산기는 안정 부하 세 앱에 적용한다. 보고 앱의 램프에서는 0~255가 각각 16회 나타나므로 240 이상인 표본은 매회 256개다. 각 회차의 round_stage는 계산 게시 후 1, 양보 복귀 후 2, 실제 기상 확인 후 3이다. 실제 짧은 실행 구간, 기상·복귀 및 CRC 진행량은 기록으로 확인한다. 단독 CRC에서도 완전한 계산 결과와 실제 퀀텀 배정을 대조한다.

For the all-ready first 48 full grants, expected totals A/B/C are 11/14/23 for seed 1 and 8/17/23 for seed 2. Score all 25 sliding windows. For stride compare all 48 normalized-service prefixes against the maximum measured lease divided by its owner's weight. 최초 48회 전체 경쟁 배정에서 두 시드의 총량과 25개 이동 창을 검산한다. 스트라이드는 48개 접두 구간마다 실제 가중치별 서비스 편차를 검산한다. [Deployment contract / 배포 판정 기준](../../docs/native-proportional-applications.en-ko.md)
