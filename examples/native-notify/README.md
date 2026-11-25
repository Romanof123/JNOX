# Queued pulse records and selective event consumption

Build the C telemetry sender and Rust receiver, then package the same ELFs in either scheduling mode.

~~~sh
sdk/native-aarch64/build.sh examples/native-notify/sender.c sender.elf
sdk/native-aarch64/build-rust.sh examples/native-notify/receiver/Cargo.toml receiver.elf
python3 tools/build_native_el0.py --out notify-fifo --policy fifo --priority 7 --peer-priority 1 --elf sender.elf --peer-elf receiver.elf --notify-route 0:1
python3 tools/build_native_el0.py --out notify-rr --policy rr --quantum-ticks 625000 --priority 7 --peer-priority 1 --elf sender.elf --peer-elf receiver.elf --notify-route 0:1
~~~

The higher-priority sender fills all four pulse slots, attempts a fifth, sets event masks 9, 1 and 4, then exits 61. The receiver drains the committed notifications and exits 62. The expected queue occupancy starts at zero, reaches four, stays at four on the refused fifth send, and ends at zero. The event latch moves through 0, 9, 9, 13 before any consume. Taking bit 1 leaves 12, a second try for 1 leaves 12, and taking mask 12 clears it.

| Private symbol | Expected result |
|---|---|
| Sender notify_send_status | 0, 0, 0, 0, -11, 0, 0, 0 |
| Receiver notify_codes | 17, 17, 23, 23 |
| Receiver notify_words | Upper 32 bits carry sequence 1?4; lower 32 bits carry readings 300, 512, 257, 1025 |
| Receiver notify_channel_sums | 812 for channel 17, 1282 for channel 23 |
| Receiver notify_empty_pulse | -11 after all four records |
| Receiver notify_event_results | 1, -11, 12, -11 for masks 1, 1, 12, and all bits |

The receiver checks every full-width word before summing its channel. Inspect the kernel journal for actual occupancy and latch transitions and the terminal process-local mappings for the application results.
