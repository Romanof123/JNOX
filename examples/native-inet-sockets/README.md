# Ordinary IPv4 TCP and UDP peer fixture

Use Python 3.10+, Linux QEMU user networking and filter-dump. Slot 0 runs the C TCP application, slot 1 the Rust UDP client and slot 2 the separate Rust IPv4 service. From the integrated repository root:

~~~sh
app=examples/native-inet-sockets
sh sdk/native-aarch64/build.sh --local-socket "$app/tcp.c" inet-tcp.elf
sh sdk/native-aarch64/build-rust.sh --local-socket "$app/udp/Cargo.toml" inet-udp.elf
sh sdk/native-aarch64/build-rust.sh services/inet/Cargo.toml inet-service.elf
package() {
    policy=$1
    shift
    python3 tools/build_native_el0.py --out "inet-$policy" --policy "$policy" --elf inet-tcp.elf --priority 4 --peer-elf inet-udp.elf --peer-priority 4 --peer-elf inet-service.elf --peer-priority 7 --peer2-arg 3 --ipc-route 0:2 --ipc-route 1:2 --socket-service 2 --net-service 2 "$@"
}
package fifo
package rr --quantum-ticks 625000
python3 "$app/peer.py" inet-fifo inet-fifo-first
python3 "$app/private_results.py" inet-fifo inet-fifo-first inet-tcp.elf inet-udp.elf
python3 "$app/peer.py" inet-rr inet-rr-first
python3 "$app/private_results.py" inet-rr inet-rr-first inet-tcp.elf inet-udp.elf
~~~

Preserve each first outcome. Before starting QEMU, the peer binds TCP 43002 and UDP 43004 to loopback. The guest sees 10.0.2.2; its own address is 10.0.2.15. The only inbound forward is 127.0.0.1:43001 to guest TCP 43001. The runner creates a fresh directory and captures peer.jsonl, device-side Ethernet wire.pcap, argv/kernel hash, UART, status and post-stop 128 MiB RAM. Keep the manifest and all three exact ELFs. The private decoder uses the project's pyelftools replay support and GNU AArch64 nm.

* **Outgoing TCP.** The guest creates its listener before connecting to host 43002. It sends GUEST/out, half-closes and receives HOST/outbound in reads of at most three bytes, then EOF. The peer verifies the original request bytes and FIN.
* **Incoming TCP.** The host connects through the loopback forward, sends HOST/ followed by inbound, then half-closes. The guest accepts, verifies 12 bytes in reads of at most four, reads EOF and sends GUEST/accepted. The host verifies 14 bytes and EOF. Read boundaries may vary.
* **Reset.** The guest makes a separate connection and sends ARM!. Only after receipt does the host close its ordinary socket with SO_LINGER={1,0}. The guest's private result must show recv=-1, errno 104 and clean close. Check the RST on the guest-facing flow. If user networking presents EOF, preserve exit 112 and both observed values; host abortive close alone cannot establish application-visible reset.
* **UDP relay.** Guest port 43003 sends four three-byte records (U,id,10×id) to host 43004. The peer logs original bytes and socket source tuple before its relay decision. It deliberately drops ID 2, sends (R,id,10×id) for IDs 1,4,3 in order, then ID 0 as an end marker. The guest validates each endpoint/payload and polls for unexpected records for a further 100 ms of guest monotonic time, then sends DONE. Compare pre-relay and post-send records with the independent device pcap. The absent record is a declared relay decision.
* **Expected receipts.** TCP slot 0 exits 75, stage 4, outbound counts 9/13, inbound counts 12/14, both EOF reads 0 and reset −1/104. UDP slot 1 exits 76, source 0x0a000202:43004, receive order 1,4,3,0, positive empty post-marker polls and close flag 1. Service slot 2 exits 0. Partial results remain inspectable in RAM. Physical DMA and link qualification require separate evidence.

Inspect traffic with tcpdump -nn -tt -r inet-fifo-first/wire.pcap 'tcp or udp'. Match endpoints, flags, direction and payloads to the host log; TCP segments and translated source ports can differ from user operations.

For a separate direct-peer position, provision a dedicated Linux network namespace with only loopback and a user-owned TAP. These Linux commands need iproute2, tuntap and namespace privileges. Use a fresh namespace and output directory; assign no physical interface or default route.

~~~sh
ns=jnox-inet-fixture
sudo ip netns add "$ns"
sudo ip -n "$ns" link set lo up
sudo ip netns exec "$ns" ip tuntap add dev jnx0 mode tap user "$(id -u)"
sudo ip -n "$ns" addr add 10.0.2.2/24 dev jnx0
sudo ip -n "$ns" link set jnx0 up
sudo ip netns exec "$ns" runuser -u "$(id -un)" -- python3 "$app/peer.py" inet-fifo inet-fifo-tap-first --isolated-tap jnx0
sudo ip netns del "$ns"
~~~

Here the host binds 10.0.2.2 and connects directly to 10.0.2.15:43001. Preserve that configuration's capture and outcome separately from the user-networking first position.
