# Native extension sensor programs / 네이티브 확장 센서 프로그램

From the repository root in Linux or WSL, build the three private ELFs once and reuse them. Linux 또는 WSL의 저장소 루트에서 세 ELF를 한 번 빌드한 뒤 모든 구성에 재사용한다.

~~~sh
src=examples/native-extension
out=output/native-extension
mkdir -p "$out"
sh sdk/native-aarch64/build.sh "$src/voltage.c" "$out/voltage.elf"
sh sdk/native-aarch64/build-rust.sh "$src/temperature/Cargo.toml" "$out/temperature.elf"
sh sdk/native-aarch64/build.sh "$src/alarms.c" "$out/alarms.elf"
package() {
    name=$1
    rule=$2
    shift 2
    python3 tools/build_native_el0.py --out "$out/$name" --policy extension \
        --elf "$out/voltage.elf" --peer-elf "$out/temperature.elf" --peer-elf "$out/alarms.elf" \
        --extension-program "$src/rules/$rule.txt" "$@"
}
for rule in accepted stale cpu fuel; do package "$rule" "$rule"; done
package extended accepted --peer2-arg four-batches
package sleep accepted --arg sleep-400

qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel "$out/accepted/kernel.elf"
~~~

Use each package's kernel.elf for its own cold launch. Freeze the binaries, manifests and QEMU command before the first plain and independently traced positions. Use a fresh output path for each -d int -D logfile. 각 구성의 kernel.elf를 콜드 부팅한다. 일반·외부 추적 최초 부팅 전에 ELF, 매니페스트, QEMU 명령을 고정하고 예외 로그를 별도 경로에 보관한다.

| Slot / 슬롯 | Per-batch sample counts / 배치별 표본 수 | Batch products / 배치별 결과 | Cumulative products / 누적값 | Exit / 종료 |
| --- | --- | --- | --- | ---: |
| 0: voltage, reserved / 예약 | 2, 1 | 6596, 3304 | 6596, 9900 | 30 |
| 1: temperature, delegated / 위임 | 1, 1, 1 | 18, 23, 21 | 18, 41, 62 | 31 |
| 2: alarms, delegated / 위임 | 1048576 each / 매회 | 65536 each / 매회 | 65536, 131072, 196608; fourth / 네 번째 262144 | 32 |

In each ELF, private_batches (u32) gives the valid prefix of batch_count[4] (u32), batch_sum[4] and cumulative_sum[4] (both u64). private_result (u64) holds the final cumulative value. Samples are published before yielding or exiting. The alarms result follows from 4096 ramps per batch × 16 qualifying bytes. 각 ELF의 사설 매핑에서 완료 개수 private_batches만큼 배열 앞부분을 읽는다. 세 프로그램은 양보나 종료 전에 결과를 게시한다. 경보 한 배치의 값은 램프 4096개 × 임계값 이상 바이트 16개다.

For cells 0–7 the accepted rule predicts 2,2,0,2,1,0,1,1. The stale generation, wrong CPU and five-step fuel cases predict 1,1,0,1,2,0,2,2. In the extended run check that cell 8 stays idle and the remaining delegate finishes in cell 9. In the sleep run inspect slot 0's u64 request_ns and observed_ns; the next reserved opening should be idle and the actual return must reach the absolute target. These are predictions to compare with the recorded frames, timers and private products. 정상 추천과 세 폴백 규칙의 셀별 순서를 실제 기록과 대조한다. 추가 경보 배치에서는 셀 8 전체가 유휴인지, 마지막 위임 앱이 셀 9에서 끝나는지 확인한다. 수면 구성은 전압 앱이 첫 배치 후 400 ms 절대 대기를 요청한다. 다음 예약 셀의 유휴와 실제 기상 시각을 확인한다. [Deployment criteria / 배포 판정 기준](../../docs/native-extension-applications.en-ko.md)
