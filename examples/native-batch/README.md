# Native batch jobs / 네이티브 배치 작업

Build the five ordinary C applications once from the repository root under Bash on Linux or WSL. Each job tuple is (release offset from epoch E, estimated service), in physical counter ticks. At 62,500,000 Hz, 3,125,000 ticks are 50 ms. Linux 또는 WSL의 Bash에서 아래 ELF 다섯 개를 한 번 빌드한다. 튜플은 물리 카운터 틱 단위의 (공통 E 기준 최초 공개 오프셋, 서비스 추정치)다.

~~~bash
out=output/native-batch
mkdir -p "$out"
for app in batch urgent sleep_first sleep_last; do
    sh sdk/native-aarch64/build.sh "examples/native-edf/$app.c" "$out/$app.elf"
done
sh sdk/native-aarch64/build.sh examples/native-llf/checksum.c "$out/checksum.elf"
pack=(python3 tools/build_native_el0.py)
a=(--elf "$out/batch.elf")
fixed=(--arg=--blocks --arg 32768)
b=(--peer-elf "$out/urgent.elf")
c=(--peer-elf "$out/checksum.elf")

for policy in sjf srtf hrrn; do
    "${pack[@]}" --out "$out/arrival-$policy" --policy "$policy" \
        "${a[@]}" "${fixed[@]}" --batch-job 0,18750000 "${b[@]}" --peer-batch-job 6250000,625000
    "${pack[@]}" --out "$out/tie-$policy" --policy "$policy" \
        "${a[@]}" --batch-job 0,1 "${b[@]}" --peer-batch-job 3125000,625000 \
        "${c[@]}" --peer-batch-job 3125000,625000
done
for policy in hrrn sjf; do
    "${pack[@]}" --out "$out/aging-$policy" --policy "$policy" \
        "${a[@]}" --batch-job 0,1 "${b[@]}" --peer-batch-job 0,3125000 \
        "${c[@]}" --peer-batch-job 9375000,625000
done
"${pack[@]}" --out "$out/underestimate" --policy srtf "${a[@]}" "${fixed[@]}" \
    --batch-job 0,1 "${b[@]}" --peer-batch-job 6250000,1
"${pack[@]}" --out "$out/lifecycle" --policy srtf --elf "$out/sleep_first.elf" \
    --batch-job 0,1 --peer-elf "$out/sleep_last.elf" --peer-batch-job 0,1
"${pack[@]}" --out "$out/solo" --policy sjf --elf "$out/urgent.elf" --batch-job 3125000,625000
~~~

Launch any package using its kernel.elf on one default-clock Cortex-A72. 각 구성의 kernel.elf를 기본 시계의 단일 Cortex-A72에서 부팅한다.

~~~bash
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio \
    -kernel "$out/arrival-srtf/kernel.elf"
~~~

| ELF and arguments / 앱과 인수 | Private product / 사설 결과 | Exit / 종료 |
| --- | --- | ---: |
| batch --blocks 32768 | private_progress=32768, private_result=17112760320 (u64) | 17 |
| batch, no extras / 추가 인수 없음 | private_progress=n, private_result=522240×n (u64); 160 ms minimum / 최소 160 ms | 17 |
| urgent | private_progress=8, private_result=2048 (u32) | 43 |
| checksum | private_blocks=16, private_result=8355840 (u64) | 17 |
| sleep_first / sleep_last | private_progress=2; private_result=53 / 59 (u32); observed_ns ≥ request_ns | 53 / 59 |

Each 4096-byte ramp contains sixteen complete byte permutations, giving sum 522240 and 256 samples ≥240. Counts are u32. Identify each process by slot and packaged ELF. 4096바이트 램프에는 0~255 순열이 16회 있어 블록 합은 522240, 240 이상인 값은 256개다. 처리 블록 수는 u32이며 슬롯과 ELF로 각 결과를 식별한다.

* **Arrival / 도착:** Capture A in batch_reduce at the release of B; compare SRTF's actual A→B→A frame restoration with continued A ownership under SJF and HRRN. B 공개 시 실제 중단 PC와 A 재개 뒤 진척을 확인한다.
* **Aging / 노화:** At A's exit time t the distinguishing range is 9375000 ≤ t−E < 11718750. HRRN then dispatches B, SJF chooses shorter C. Record a missed range explicitly. 시각이 범위를 벗어나면 노화 순서 구분을 미확인으로 기록한다.
* **Ties and underestimate / 동률·과소 추정:** For the three-way case B and C arrive together; check A finishes, then B and C run in admission order. For SRTF's one-tick estimates require A still running at residual one when B's tie arrives. 같은 순위에서 실행자가 유지되고 대기자는 도착 순서를 따르는지 확인한다.
* **Lifecycle and solo / 수명·단독:** Observe the actual voluntary yield, 40/140 ms waits, idle IRQ return, elapsed sleep and final sole yield. The solo alarm first enters at or after E+3125000. 양보·대기·유휴 복귀 및 단독 앱의 지연 공개는 실제 카운터로 확인한다.

Retain plain and externally traced arrival positions per policy, including any missed observation, together with private RAM, ELF/disassembly and the original journals. 정책별 도착 사례의 최초 일반·예외 추적 원본과 프로세스별 RAM, ELF, 역어셈블리를 보관한다. [Batch contract / 배치 판정 기준](../../docs/native-batch-applications.en-ko.md)

For an SRTF arrival capture and offline replay, keep independent new directories for the plain and traced positions. Run from the same source revision used to build. SRTF 도착 사례의 일반·추적 실행은 빌드한 리비전에서 각각 새 경로에 수집한다.

~~~bash
pkg="$out/arrival-srtf"
revision=$(git rev-parse HEAD)
for mode in plain traced; do
    cap="$out/capture-arrival-srtf-$mode"
    flags=()
    if [ "$mode" = plain ]; then flags=(--plain); fi
    python3 tools/native_el0_capture.py "$cap" "$pkg/kernel.elf" "$pkg/user.elf" \
        examples/native-edf/batch.c "$revision" --manifest "$pkg/manifest.json" \
        --expect-exit 17 "${flags[@]}" || break
    digest=$(sha256sum "$cap/session.json" | cut -d' ' -f1)
    python3 tools/native_batch_replay.py "$pkg" "$cap" --session-sha256 "$digest" --products batch,urgent
done
~~~

Retain the session digest with the capture and use the retained digest for later replay. Use the slot-ordered product lists batch,urgent,checksum for aging/ties, sleep_first,sleep_last for lifecycle, and urgent for solo; change the primary C source and expected primary exit accordingly. 최초 세션 해시를 별도 보관해 오프라인 검증에 사용한다. 재생 시 제품명은 슬롯 순서로 지정하고 수집 시 주 ELF에 맞는 C 소스와 종료값을 사용한다.
