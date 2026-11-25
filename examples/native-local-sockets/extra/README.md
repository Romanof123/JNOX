# Local socket supplemental guest positions (#381)

From the repository root, build the two ordinary applications and the broker.
Run each of the three positions once on FIFO and once on RR, retaining the first
outcome. Slot 0 (C owner) and slot 1 (Rust peer) each have a synchronous route
to the broker in slot 2. The two additional direct routes carry only start and
completion handshakes. Those handshakes do not enter the broker.

~~~sh
extra=examples/native-local-sockets/extra
sh sdk/native-aarch64/build.sh --local-socket "$extra/owner.c" owner.elf
sh sdk/native-aarch64/build-rust.sh --local-socket "$extra/peer/Cargo.toml" peer.elf
sh sdk/native-aarch64/build-rust.sh services/socket/Cargo.toml broker.elf
package() {
    kind=$1
    policy=$2
    shift 2
    python3 tools/build_native_el0.py --policy "$policy" --out "extra-$kind-$policy" \
        --elf owner.elf --arg "$kind" --priority 7 \
        --peer-elf peer.elf --peer-arg "$kind" --peer-priority 4 \
        --peer-elf broker.elf --peer2-arg 3 --peer-priority 1 --socket-service 2 \
        --ipc-route 0:2 --ipc-route 1:2 --ipc-route 0:1 --ipc-route 1:0 "$@"
}
for kind in backlog expired ready; do
    package "$kind" fifo
    package "$kind" rr --quantum-ticks 625000
done
~~~

Use the existing #376 single-core Cortex-A72 QEMU virt raw RAM/UART capture and
register trace. Expected exits are 81, 82 and 0. Read extra_owner_result and
extra_peer_result as 16 little-endian u64 words from their respective private
roots. Preserve their values on failure; exit alone does not establish ordering.

* **backlog**: slot 0 listens with backlog two. Slot 1 attempts three ordinary
  blocking connects and sends marker 51, 52 or 53 on each resulting stream.
  The listener sleeps until the planned first accept, then accepts and reads
  each stream. Peer words 0..5 hold before/after samples around each connect;
  owner words 10..12 hold samples before accept. Owner words 4..6 must be
  51,52,53. Require trace evidence of two actual queued connections, a third
  blocked COMMIT held without a reply, the first accept freeing space and one
  successful reply to the matching token. If the guest missed that ordering,
  retain the counts and mark the promotion unobserved.
* **expired**: the C client stages a one-byte datagram receive and issues a
  finite raw COMMIT (absolute monotonic deadline 200 ms after setup). Rust
  waits until 50 ms after that deadline before sending its one record, 0x6d.
  Following timeout the client waits on direct IPC. It sends **no** broker RPC,
  including ABORT, until Rust reports that the broker has completed the send.
  The first new nonblocking receive must then obtain exactly that byte, and
  the next must return EAGAIN. Owner words 0..3 are deadline, raw status
  (-110 as u64), reply (0), and actual return sample. Words 4..7 must be
  1,109,-1,11. Word 10 records errno if the first receive failed. Require
  broker-side evidence that the expired token did not consume or stage the
  sent record, and no later reply was delivered to the expired token.
* **ready**: the same send is requested 100 ms before the deadline. The pending
  raw COMMIT must complete with status 0 and word 1 before expiry. After the
  direct handshake the owner reads actual META/READ words (indices 8 and 9).
  Copied and original lengths must both be 1 and the byte must be 0x6d. A
  fresh receive must return EAGAIN. Verify one broker commit, one reply and
  one consumed record. For both timer positions peer words 0..3 retain the
  deadline, before/after send samples and send count. Distinguish a missed
  timing position from a witnessed stale consume or duplicate delivery.
