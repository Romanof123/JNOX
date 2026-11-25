# Copied telemetry words

From the repository root, build the two native ELFs and grant slot 0 permission to send to slot 1.

~~~sh
sdk/native-aarch64/build.sh examples/native-mailbox/sender.c sender.elf
sdk/native-aarch64/build-rust.sh examples/native-mailbox/receiver/Cargo.toml receiver.elf
python3 tools/build_native_el0.py --out mailbox-fifo --policy fifo --priority 7 --peer-priority 1 --elf sender.elf --peer-elf receiver.elf --mailbox-route 0:1
python3 tools/build_native_el0.py --out mailbox-rr --policy rr --quantum-ticks 625000 --priority 7 --peer-priority 1 --elf sender.elf --peer-elf receiver.elf --mailbox-route 0:1
~~~

The high-priority C sender fills four slots before its first blocking call. A nonblocking fifth word is refused. A different fifth word has an absolute monotonic deadline 100 ms after the sender's clock sample. The lower-priority Rust receiver sleeps until 400 ms after its own start. On timeout the sender submits a final word with an indefinite wait. The receiver's first dequeue frees capacity for that word. With this schedule the sender can exit before the receiver drains the remaining committed words.

| Private ELF symbol | Expected terminal contents |
|---|---|
| Sender mailbox_send_status | 0, 0, 0, 0, -11, -110, 0 |
| Sender mailbox_send_times | Start, absolute deadline, resumed time at or after deadline |
| Receiver mailbox_received | Sequence numbers 1?5 in the upper 32 bits, readings 300, 512, 257, 1025, 88 in the lower 32 bits |
| Receiver mailbox_reading_sum | 2182 |
| Receiver mailbox_empty_status | -11 |

Expected exits are 51 (sender) and 52 (receiver). Check the guest dispatch and mailbox journal for occupancy four, a genuinely blocked timed send, cancellation, and the later promotion after a slot opens. The statuses and private words alone cannot distinguish expiration on syscall entry from expiration after blocking. An unusually delayed guest can miss a planned causal observation; retain its original capture.
