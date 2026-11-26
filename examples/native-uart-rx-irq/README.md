# PL011 receive interrupts / PL011 수신 인터럽트

This Rust application exercises a QEMU virt PL011 through the JNOX native display-owner API. An independent host sends eight binary bytes, waits for the guest's reply, then sends 24 bytes into the empty 16-byte ring. The application sleeps on the native timer and queries IRQ statistics before it dequeues any data. A single ramfb scanline reports the returned bytes, IRQ counts, queue depth, drops and elapsed guest milliseconds. QEMU reads that row independently of the serial stream under test.

Rust 앱이 타이머로 잠든 사이 호스트가 실제 시리얼 입력을 보낸다. 앱은 인터럽트 처리 건수를 확인한 뒤 수신 큐를 읽는다. 가상 화면의 첫 줄에 통계와 실제 수신 바이트를 표시하므로 호스트 검증기가 시리얼 채널과 독립적으로 판독할 수 있다.

From a clean Linux checkout with the AArch64 native SDK, run the three packages sequentially.

```sh
set -eu
b=$(mktemp -d)
export CARGO_BUILD_JOBS=1
sh sdk/native-aarch64/build-rust.sh examples/native-uart-rx-irq/Cargo.toml "$b/app.elf"
for mode in irq polled; do
    set --
    if [ "$mode" = irq ]; then set -- --uart-rx-irq; fi
    python3 tools/build_native_el0.py --board virt --policy fifo \
        --elf "$b/app.elf" --priority 1 --peer-elf "$b/app.elf" --peer-priority 2 \
        --peer-arg denied --display-service 0 "$@" --out "$b/$mode"
done
python3 tools/build_native_el0.py --board virt --policy fifo \
    --elf "$b/app.elf" --priority 1 --out "$b/headless"
python3 tools/verify_native_uart_rx_irq.py --package "$b/irq" --polled-package "$b/polled" \
    --headless-package "$b/headless" --out "$b/capture"
python3 tools/verify_native_uart_rx_irq.py --replay "$b/capture"
```

The IRQ case expects all eight initial bytes in order. On the second transmission the receiver keeps the first 16 bytes, drops eight, and accepts a final `q` after the queue drains. The peer must be denied both direct RX and IRQ statistics. The default driver must still receive a polled byte, and a headless image must report that the interface is unavailable. The observer stores serial input/output, QMP frames, process exits, packaged images and hashes for replay. Timer intervals are guest observations on one virtual CPU.

첫 시험에서는 8바이트의 순서를 대조하고, 다음 시험에서는 24바이트 중 처음 16바이트가 도착하고 나머지 8바이트가 유실로 계수되는지 확인한다. 큐를 비운 뒤에는 후속 입력도 수신해야 한다. 기존 폴링 방식과 화면 서비스가 없는 이미지도 같은 앱으로 검증한다. 관측 대상은 단일 가상 CPU다.
