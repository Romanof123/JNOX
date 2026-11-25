# Native mixed-criticality sensor workers / 혼합 중요도 센서 작업

Build the slot-specific ELFs once from the repository root. All four share the published ring and reduction code; each returns its slot ID on success. Each release starts with clean private memory. 저장소 루트에서 네 ELF를 한 번 빌드한다. 슬롯별 작업은 같은 링과 계산 코드를 쓰며 성공하면 슬롯 번호로 종료한다.

~~~bash
out=output/native-amc
mkdir -p "$out"
for slot in 0 1 2 3; do
    sh sdk/native-aarch64/build.sh "examples/native-amc/slot$slot.c" "$out/slot$slot.elf"
done
pack=(python3 tools/build_native_el0.py --policy amc --elf "$out/slot0.elf" --peer-elf "$out/slot1.elf")
more=(--peer-elf "$out/slot2.elf" --peer-elf "$out/slot3.elf")
for name in within-lo refusal overlap; do
    "${pack[@]}" --out "$out/$name" --amc-manifest "examples/native-amc/config/$name.json"
done
for name in normal-hi over-hi staggered; do
    "${pack[@]}" "${more[@]}" --out "$out/$name" --amc-manifest "examples/native-amc/config/$name.json"
done
~~~

config/refusal.json is a valid package containing runtime reset and release refusals. Separately submit config/invalid-budget.json with the two-ELF command to exercise prelaunch rejection; adding a fifth ELF to the four-slot normal-hi command must also be refused. 고의로 잘못된 예산과 다섯 번째 ELF는 부팅 전에 거절되어야 한다. refusal 구성에서는 부팅한 뒤 실제 재시작·릴리스 거절을 확인한다.

Each argv list contains three extra strings: accepted generation index (starting at 0), mode, and amount. Finite mode stores 8 or 16 actual readings. Stream mode adds batches of 256 volatile loads, checks its own CLOCK_PROCESS_CPUTIME_ID every 256 batches and finishes once generation CPU service reaches the requested milliseconds. Survivor mode also publishes an eight-sample checkpoint before streaming and a separate sixteen-sample product afterwards. Clock samples use the ordinary SDK. No yield or sleep controls this workload.

인수 세 개는 수용 세대 번호(0부터), 모드, 개수 또는 CPU 밀리초다. 스트림은 256개 표본마다 합을 게시하고 256배치마다 자기 CPU 시계를 읽는다. 생존 작업은 스트림 전 8개, 완료 후 16개 표본도 별도로 남긴다.

Private symbol amc_product is a 936-byte little-endian record, initially zero. FINAL means the successful path published its last clock samples before returning from main. A discard or exhaustion can retain an earlier complete prefix.

| Byte offset | Field / 필드 |
| ---: | --- |
| 0,4,8,12 | u32 version (1), slot, generation, active buffer index |
| 16 | Two 16-byte buffers of u64 count and sum; index active selects the last committed pair |
| 48 | Sixteen u32 finite reading values |
| 112,116 | u32 checkpoint_count and finite_count |
| 120,128 | u64 checkpoint_sum and finite_sum |
| 136,140 | u32 stamp_count and stamp_overflow |
| 144 | 32 stamps, 24 bytes each: u64 monotonic_ns, cpu_ns, completed count |
| 912 | FINAL stamp with the same fields |

Use the published active buffer even when the worker was interrupted during its next batch. Stamps are recorded at entry and at observations spaced at least 8 ms of process CPU time apart; final_stamp is separate. For slot i, generation g, compare each finite value against ring[(i+g+j) mod 8] for j from zero. Independently compute the count/sum pair as 36 × floor(count/8) plus the rotated ring prefix modulo 2⁶⁴. Fresh LO's eight readings sum to 36; the staggered survivor's checkpoint and final product sum to 36 and 72. Compare each task's actual completion syscall, wall-clock and CPU samples to the recorded generation, cutoff and deadline.

폐기·소진된 작업은 마지막으로 게시된 버퍼의 표본 수와 합을 읽는다. 링은 [2,5,1,6,3,7,4,8]이며 슬롯과 세대 번호의 합으로 시작 위치를 회전한다. 모든 8표본 합은 36, 16표본 합은 72다. 모드 전환·폐기·재시작·마감 여부는 실제 CPU 시간, 작업 시각 표본과 예외 기록에서 판정한다. [Cases and observation criteria / 실행 및 판정 기준](../../docs/native-amc-applications.en-ko.md)
