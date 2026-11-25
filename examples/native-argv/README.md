# Native sensor argument examples

From the repository root under Bash on Linux or WSL, build the C application once for both C slots and build the independent Rust application for slot 2. Use a UTF-8 locale and fresh output directories.

~~~bash
sh sdk/native-aarch64/build.sh examples/native-argv/sensor.c sensor.elf
sh sdk/native-aarch64/build-rust.sh examples/native-argv-rust/Cargo.toml sensor-rust.elf

primary=(--elf sensor.elf --arg sum --arg 16 --arg 32 --arg 'west rack')
peer=(--peer-elf sensor.elf --peer-arg minmax --peer-arg 64 --peer-arg 96)
pack=(python3 tools/build_native_el0.py)
"${pack[@]}" --out coop "${primary[@]}" "${peer[@]}" --peer-arg ''
"${pack[@]}" --out first-peer --policy fifo --priority 1 --peer-priority 7 \
    "${primary[@]}" "${peer[@]}" --peer-arg 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'
"${pack[@]}" --out first-rust --policy fifo --priority 1 --peer-priority 2 --peer-priority 7 \
    "${primary[@]}" "${peer[@]}" --peer-arg '' --peer-elf sensor-rust.elf \
    --peer2-arg sum --peer2-arg 240 --peer2-arg 256 --peer2-arg='-동쪽 센서'
"${pack[@]}" --out default --elf sensor.elf
"${pack[@]}" --out mixed --elf sensor.elf "${peer[@]}" --peer-arg ''
"${pack[@]}" --out probe
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio \
    -kernel coop/kernel.elf
~~~

Use the selected package's kernel.elf for each boot. The C application accepts sum and minmax; Rust accepts sum. Endpoints are nonempty ASCII decimal integers satisfying 0 ≤ low < high ≤ 256. Both applications check argc, the argv and empty envp NULL sentinels, and bounded argument strings. Labels remain opaque bytes. Both check a volatile stack local and the original label bytes through its NUL after every four samples and publish the number of successful returns.

| Slot input | Bytes in label | Count | Product | Yield returns | Exit |
| --- | ---: | ---: | --- | ---: | ---: |
| C sum [16,32), west rack | 9 | 16 | sum 376 | 4 | 17 |
| C minmax [64,96), empty or 32 x bytes | 0 or 32 | 32 | min 64, max 95 | 8 | 29 |
| Rust sum [240,256), -동쪽 센서 | 14 | 16 | sum 3960 | 4 | 43 |
| C default /bin/app alone | absent | 0 | no samples requested | 0 | 0 |

Read sensor_argc, sensor_lengths[16] (u32 byte lengths), sensor_label[1024] (bytes), sensor_count, sensor_yields (u32), sensor_sum (u64), plus C sensor_min and sensor_max (u32) in each process's active mapping. The C default vector has argc 1 and first length 8. Customized vectors have argc 5. Match each vector and its _start SP with the [startup layout and acceptance cases](../../docs/native-process-arguments.en-ko.md).
