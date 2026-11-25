# Native hierarchical telemetry / 네이티브 계층형 텔레메트리

From the repository root on Linux or WSL, build the four existing applications once. Use the same ELF files in all packages. Slots 0, 1 and 2 map to leaves 0, 1 and 2; each JSON row gives the parent index and quota in architectural counter ticks.

Linux 또는 WSL의 저장소 루트에서 기존 앱 네 개를 한 번 빌드한 뒤 아래 네 패키지에서 같은 ELF들을 재사용한다. 슬롯 0·1·2는 같은 번호의 리프에 연결된다. JSON의 자식·리프 행은 소속 상위 노드 번호와 카운터 틱 단위 예산이다.

~~~sh
out=output/native-hierarchy
mkdir -p "$out"
sh sdk/native-aarch64/build.sh examples/native-weighted-analytics/checksum.c "$out/crc.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-weighted-analytics-rust/Cargo.toml "$out/histogram.elf"
sh sdk/native-aarch64/build.sh examples/native-weighted-analytics/energy.c "$out/energy.elf"
sh sdk/native-aarch64/build.sh examples/native-weighted/reporter.c "$out/reporter.elf"

package() {
    python3 tools/build_native_el0.py --out "$out/$1" --policy hierarchy \
        --hierarchy "examples/native-hierarchy/$2.json" --elf "$out/$3.elf" \
        --peer-elf "$out/$4.elf" --peer-elf "$out/$5.elf"
}
package saturated  baseline   crc      histogram energy
package swap       baseline   energy   histogram crc
package child-leaf child-leaf crc      histogram energy
package wake       baseline   reporter histogram energy

qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio \
    -kernel "$out/saturated/kernel.elf"
~~~

At 62,500,000 Hz the period is 250 ms. In saturated and swap, leaves 0 and 1 share the 3,906,250-tick budget of parent 0 while leaf 2 uses parent 1. In child-leaf, child 0 has 1,953,125 ticks and leaf 1 has 976,562. In wake, the reporter sleeps three times with the baseline quotas. Boot each named package plain and then externally traced in the listed order, using its own kernel.elf and fresh capture paths; add -d int -D logfile for the traced boot.

62,500,000 Hz에서 한 주기는 250 ms다. 포화·교환 구성에서는 리프 0과 1이 부모 0의 3,906,250틱을 공유하고 리프 2는 부모 1을 사용한다. 하위 제한 구성에서는 자식 0이 1,953,125틱, 리프 1이 976,562틱을 받는다. 깨우기 구성에서는 기준 예산에서 보고 앱이 세 번 잔다. 표기된 순서대로 각 패키지의 kernel.elf를 일반 부팅 후 별도 경로에 외부 추적 부팅하고, 추적 부팅에 -d int -D 로그파일을 추가한다.

| ELF | Final private_progress / 최종 진행량 | Final private_result / 최종 계산값 | Exit / 종료값 |
| --- | ---: | --- | ---: |
| crc | 134217728 bytes | CRC-32 2124873618 | 237 |
| histogram | 536870912 bytes | 2097152 × [1,8,28,56,70,56,28,8,1] | 70 |
| energy | 1073741824 samples | Sum of squares / 제곱합 5864240971776 | 179 |
| reporter | 3 reports | Mask / 완료 비트 7 | 7 |

Read each symbol through its ELF's private mapping. For the reporter compare each of three u64 report_observed_ns samples with the corresponding report_deadline_ns after its 40 ms absolute wait. Independently reproduce the products with `python3 examples/native-weighted-analytics/oracle.py`. For timing and quota decisions use the [hierarchy deployment criteria](../../docs/native-hierarchical-applications.en-ko.md).

각 ELF의 사설 매핑에서 결과를 읽는다. 보고 앱은 40 ms 절대 대기마다 report_observed_ns의 실제 관측값이 해당 report_deadline_ns 이상인지 세 번 모두 확인한다. 위 Python 명령으로 각 계산값을 독립 재현하고, 시간·예산 판정에는 [계층형 배포 기준](../../docs/native-hierarchical-applications.en-ko.md)을 적용한다.
