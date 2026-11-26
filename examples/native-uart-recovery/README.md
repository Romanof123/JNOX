# One-shot PL011 driver recovery / PL011 드라이버 1회 교체

A native Rust ELF runs in three private process slots on AArch64 QEMU virt. The primary owns PL011 and ramfb. A peer checks that its direct input and statistics calls are denied on each tick. A dormant standby can acquire the device once, after the primary reports failure by exiting with status 86. The normal-exit control and the recovery case boot the same image. [Lifecycle issue #415](https://github.com/Romanof123/JNOX/issues/415)

화면 소유자, 계속 실행되는 앱, 대기 드라이버가 분리된 주소 공간을 쓴다. 검증자는 게스트에 실제 시리얼 입력을 보내고, 입력 관측과 독립된 QMP 화면·메모리 읽기로 교체 과정을 확인한다. 86은 앱이 직접 오류를 보고하는 종료 코드다.

```sh
b=$(mktemp -d)
sh sdk/native-aarch64/build-rust.sh examples/native-uart-recovery/Cargo.toml "$b/app.elf"
python3 -B tools/build_native_el0.py --board virt --policy fifo \
  --elf "$b/app.elf" --priority 1 --peer-elf "$b/app.elf" --peer-priority 2 --peer-arg peer \
  --peer-elf "$b/app.elf" --peer-priority 3 --peer2-arg standby --display-service 0 \
  --uart-rx-irq --uart-rx-standby 2 --out "$b/pkg"
python3 -B tools/verify_native_uart_recovery.py --package "$b/pkg" --out "$b/capture"
python3 -B tools/verify_native_uart_recovery.py --replay "$b/capture"
```

QEMU inspection uses `aarch64-linux-gnu-nm` to find the private probe in the frozen ELF. The observer walks each process's tables and samples its private state. It reads the PL011 interrupt mask and the GICv2 enable bit for INTID 33. During recovery the primary leaves old input unread. The observer then injects byte `d3` with UART and GIC interrupts masked, samples UARTFR.RXFE clear while the hardware holds it, and samples RXFE set after the replacement initializes. The standby starts with the device masked, observes an empty queue and reset counters after initialization, returns eight fresh serial bytes, then reports a second failure. The working peer's counter advances across the transition and after revocation. Normal exit dismisses the spare without running it. Hardware timing, DMA and general exception recovery have separate criteria.

구 드라이버의 읽지 않은 데이터를 남긴 뒤 예비 드라이버가 새 입력 8바이트를 반환하는지 관측한다. IRQ 차단·재활성화·최종 차단과 일반 앱의 주기 카운터 진행을 따로 검사한다. 정상 종료 시 예비 프로세스는 실행되지 않아야 한다.
