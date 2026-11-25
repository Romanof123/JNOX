# Four-process telemetry publication

The C publisher (slot 0), Rust active subscriber (1), C stalled subscriber (2)
and separately built Rust broker (3) use topic 0. The publisher has only publish
rights, the two subscribers have only subscribe rights. The broker takes the
generated masks. The ordinary direct IPC calls from 0 to 1 and 2 implement the
test barriers; each client has a separate synchronous route to broker 3.
All command replies echo the exact control word. The two subscribers reply to
START only after subscribing. Publication does not proceed to the next sequence
until the active subscriber has confirmed its actual data.

~~~sh
dir=examples/native-pubsub
sh sdk/native-aarch64/build.sh --pubsub "$dir/publisher.c" publisher.elf
sh sdk/native-aarch64/build-rust.sh --pubsub "$dir/active/Cargo.toml" active.elf
sh sdk/native-aarch64/build.sh --pubsub "$dir/stalled.c" stalled.elf
sh sdk/native-aarch64/build-rust.sh services/pubsub/Cargo.toml pubsub-broker.elf
package() {
    policy=$1
    shift
    python3 tools/build_native_el0.py --board virt --policy "$policy" "$@" --out "telemetry-$policy" \
        --elf publisher.elf --priority 4 \
        --peer-elf active.elf --peer-priority 4 \
        --peer-elf stalled.elf --peer-priority 4 \
        --peer-elf pubsub-broker.elf --peer-priority 2 \
        --pubsub-service 3 --pubsub-publish 0:1 --pubsub-subscribe 1:1 --pubsub-subscribe 2:1 \
        --ipc-route 0:1 --ipc-route 0:2 --ipc-route 0:3 --ipc-route 1:3 --ipc-route 2:3
}
package fifo
package rr --quantum-ticks 625000
~~~

Run each first on single-core AArch64 QEMU virt. Expected exits in slot order
are 71, 72, 73 and 0. Retain package hashes, UART, raw journals and terminal
RAM. Inspect each named object using its own slot ELF and private root.

* The six main publications have lengths 0, 1, 7, 32, 11 and 3. For sequence
  s and byte index j the value is (43*s + 19*j + floor(j/2)) modulo 256. Check
  actual publisher IDs, sizes and bytes against that formula and then against
  the actual active reader's arrays active_ids, active_sizes and active_data.
  In active_observations each row is [queued=1, high_water=1, lost=0, front=s].
  Both peek and the following consuming read must return the same sample.
* After sequence six, the stalled reader records queue_before=[4,4,2,1] and
  consumes exactly IDs 1..4 with the original bytes. ID 1 is a zero-length
  sample, distinct from EAGAIN on an empty queue. ID 7 (19 bytes) then probes
  recovery: the stalled reader peeks it, unsubscribes with it unread, and
  enrolls again. old_handle/new_handle retain [slot,generation]. stale_status
  is -116 and empty_status is -11 on the new handle before publication eight.
  ID 8 (six bytes) is delivered to the new handle; stalled_ids=[1,2,3,4,8]
  with original stalled_sizes and stalled_data. The active reader drains all
  eight publications. publisher_subscribe_status and wrong_publish must
  independently be -13. The refused publish cannot advance the topic sequence.
* The stalled reader exits with the replacement subscription still live. After
  replying to FINISH the active reader requests a blocking receive on its
  empty subscription. At the last publisher's exit the result must be EPIPE
  (-32, active_terminal_wait). It then unsubscribes. Verify the broker's
  terminal queues, subscriptions and deferred tokens are empty following the
  client exits. Attribute an actual blocked wake only if the trace establishes
  that order; this check also allows the publisher to have already exited.
