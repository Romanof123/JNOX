# Pi 4 partitioned native processes / Pi 4 분할 프로세스

From the repository root on a stock Linux host with the native SDK, GNU AArch64 binutils and a QEMU that lists `raspi4b`, build the three ordinary jobs. For historical comparisons reuse the exact retained #322 prime ELF bytes and check their SHA-256 before boot. Source rebuilds alone do not establish byte identity.

저장소 루트에서 아래 명령을 실행한다. 기존 #322와 비교할 때 보관된 소수 ELF 두 개의 해시를 먼저 확인한다. 달력 작업은 실제 5 ms 절대 수면 뒤 윤년 수를 계산한다.

~~~sh
sh sdk/native-aarch64/build.sh examples/native-fixed/prime_low.c low.elf
sh sdk/native-aarch64/build-rust.sh examples/native-fixed-rust/Cargo.toml high.elf
sh sdk/native-aarch64/build.sh examples/native-fixed/calendar_report.c calendar.elf

package() {
    python3 tools/build_native_el0.py --board pi4 --elf low.elf --priority 2 \
        --peer-elf high.elf --peer-priority 2 "$@"
}
package --out pair --policy smp-partitioned --cpu-mask 1 --peer-cpu-mask 2
package --out remote --policy smp-partitioned --cpu-mask 1 --peer-cpu-mask 2 \
    --peer-elf calendar.elf --peer-priority 4 --peer-cpu-mask 2 \
    --release-cell 0 --peer-release-cell 0 --peer-release-cell 1
package --out swapped --policy smp-partitioned --cpu-mask 2 --peer-cpu-mask 1
package --out legacy --policy fifo
~~~

Each negative invocation must exit nonzero before boot. Retain its full command, output and status. Use a fresh output directory for each.

~~~sh
package --out zero --policy smp-partitioned --cpu-mask 0 --peer-cpu-mask 2
package --out outside --policy smp-partitioned --cpu-mask 1 --peer-cpu-mask 4
package --out extra --policy smp-partitioned --cpu-mask 1 --peer-cpu-mask 2 \
    --peer-elf calendar.elf --peer-priority 4 --peer-cpu-mask 2 \
    --peer-elf calendar.elf --peer-priority 4 --peer-cpu-mask 1
python3 tools/build_native_el0.py --out virt-control --board virt --policy smp-partitioned \
    --elf low.elf --priority 2 --cpu-mask 1 --peer-elf high.elf --peer-priority 2 --peer-cpu-mask 2
~~~

Capture each first launch once, in order. Every position creates a new directory and prints the SHA-256 of its session record. Retain these digests separately, including for a timeout. Keep the first failure. Inspect the frozen linked disassembly before boot to determine the candidate register for each sampled arithmetic region. Use the linked instruction ranges when interpreting saved registers.

~~~sh
capture() { python3 tools/pi4_native_smp_capture.py "$@"; }
capture pair p1
capture pair p2 --trace
capture remote p3
capture remote p4 --trace
capture swapped p5
capture legacy p6
python3 tools/pi4_native_smp_replay.py p1 --session-sha256 "$P1_SHA" \
    --candidate-register "$LOW_CANDIDATE_REG" "$HIGH_CANDIDATE_REG"
~~~

Repeat replay with each retained digest. Omit candidate registers if their meaning at the sampled PCs is uncertain. Exit 2 reports missing timing/progress evidence. Compare identical raw-image digests within positions 1/2 and 3/4, and identical ordered prime ELF digests across all six. Expected private counts and native exits are 78,498/12 and 70,435/77. The remote case also requires 32/32. Match the CPU assignments against the package commands above. Preserve the #322 FIFO/RR receipts separately.

UART terminal records locate core-indexed event/frame pairs in the post-stop physical RAM snapshot. The replay walks each task's actual root to read private words. It compares the complete saved and selected frames, accounts EL0 service from return stamps, and widens cross-core comparisons using the startup counter bracket. Inspect the two disassemblies alongside the reported productive PPI PCs. The external trace count uses only lines carrying their own CPU identity. Nonzero TLS preservation needs a separate sentinel control; terminal zero TLS cannot establish it. Legacy RAM/exit checks alone cannot prove every secondary remained parked.

각 최초 부팅을 순서대로 보존하고 세션 해시를 별도로 기록한다. 재생은 CPU별 물리 인터럽트, 전체 복귀 프레임, 서비스량, 독립 페이지 테이블 및 전용 결과를 검사한다. 작업 레지스터와 실제 PC의 관계가 불확실하면 레지스터 옵션을 생략한다. CPU 간 시각 불확실성 또는 관측 누락은 미확인으로 남긴다. 호스트 실행 속도나 물리 보드 병렬성의 측정값으로 해석하지 않는다.
