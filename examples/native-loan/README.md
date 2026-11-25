# Two-entry loan ring with three 4 KiB pages

Build the C producer and Rust reader from the repository root. The pair requires a single directed route.

~~~sh
sdk/native-aarch64/build.sh examples/native-loan/producer.c producer.elf
sdk/native-aarch64/build-rust.sh examples/native-loan/reader/Cargo.toml reader.elf
python3 tools/build_native_el0.py --out loan-fifo --policy fifo --priority 7 --peer-priority 1 --elf producer.elf --peer-elf reader.elf --loan-route 0:1
python3 tools/build_native_el0.py --out loan-rr --policy rr --quantum-ticks 625000 --priority 7 --peer-priority 1 --elf producer.elf --peer-elf reader.elf --loan-route 0:1
~~~

Before the lower-priority reader runs, the producer publishes two pages and fills a third writable grant. Its third publish must return -11. It sleeps for 10 ms to let the reader drain the ring, checks that its first payload byte and excluded tail marker survived, and retries the same handle. Each subsequent frame uses the next sequence number. Eight successful publications cycle the two-entry ring four times without relying on integer counter overflow.

The producer writes byte i as 17 * sequence + 3 * i for each declared byte, and writes 0xa5 to byte 4095 outside the message. It exits 73 while holding one last unpublished grant. The reader checks every declared byte and every zero tail byte. It tries to release a previously returned handle once the same VA holds a newer grant, then verifies the new contents and releases the current handle. It exits 74 after draining the committed messages and observing -32 for an empty channel with a dead producer.

| Private result | Expected value |
|---|---|
| Producer loan_write_handles and loan_write_vas | Eight granted handles and addresses in publication order |
| Producer loan_full_status, loan_full_attempts | -11 on the third publication, at least one full-ring retry |
| Producer loan_abandoned_handle | Last acquired, unpublished handle |
| Reader loan_reads | Eight rows of read handle, read VA, x3 byte length and payload sum |
| Lengths in order | 8, 11, 14, 17, 20, 23, 26, 29 |
| Sums in order | 220, 539, 987, 1564, 2270, 3105, 4069, 5162 |
| Reader loan_stale_status, loan_final_status | Rejected stale release, then -32 after drain |

The kernel journal and page-table captures must establish the two-slot peak, each handle transition, mapping permissions, revoke-before-publication and reclamation after exit. Application results alone show the bytes and statuses that the two processes observed. The fixture never deliberately dereferences a revoked mapping.
