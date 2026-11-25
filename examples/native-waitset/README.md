# Wait for telemetry and notifications

From the repository root, build the same receiver and sender for both packages.

~~~sh
sdk/native-aarch64/build-rust.sh examples/native-waitset/receiver/Cargo.toml receiver.elf
sdk/native-aarch64/build.sh examples/native-waitset/sender.c sender.elf
python3 tools/build_native_el0.py --out waitset-fifo --policy fifo --priority 7 --peer-priority 1 --elf receiver.elf --peer-elf sender.elf --mailbox-route 1:0 --notify-route 1:0 --waitset
python3 tools/build_native_el0.py --out waitset-rr --policy rr --quantum-ticks 625000 --priority 7 --peer-priority 1 --elf receiver.elf --peer-elf sender.elf --mailbox-route 1:0 --notify-route 1:0 --waitset
~~~

Slot 0 has higher priority and enrolls for event bit 1 first. Slot 1 sends a mailbox sample, a pulse, bit 4, and finally bit 1. Only bit 1 should wake the receiver. It consumes that bit, registers the already queued mailbox, polls twice and consumes the sample once. The receiver fills all four registrations before detaching its ready pulse, so the replacement event (bit 8) must reuse the freed slot. A poll must now find nothing even though the original pulse and bit 4 remain pending. The receiver blocks again, allowing the sender to set bit 8. The receiver checks both bits separately and drains the original pulse. With all sources empty it polls once and then waits to a finite absolute monotonic deadline.

| Private symbol | Expected result |
|---|---|
| Sender waitset_sender_foreign, waitset_send_status | Foreign-owner attach refused; five successful sends/sets |
| Receiver waitset_handles | Handles for event 1, mailbox, event 16, pulse, replacement event 8. Replacement differs from the detached pulse handle. |
| Receiver waitset_observations | (event 1 handle, 1), (mailbox handle, 1) twice, (pulse handle, 1), (replacement handle, 8) |
| Receiver waitset_errors | Negative foreign-owner and stale-generation results, -11 empty replacement, -11 empty final poll, -110 deadline expiry |
| Receiver waitset_products | Mailbox word 0x000000010000012c, pulse code 17, pulse word 0x0000000200000200, reading sum 812 |

Expected exits are 71 for the receiver and 72 for the sender. Use guest scheduler and waitset records to confirm the two actual blocked-to-ready handoffs, slot reuse, and the cause of the finite expiry. Terminal process-local results establish the values observed by each app.
