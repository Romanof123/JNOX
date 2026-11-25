# Local socket telemetry exchange

Slot 0 is the ordinary C server, slot 1 the ordinary Rust client and slot 2 the separately built EL0 socket broker. Both clients select broker slot 2. The broker argument 3 admits client slots 0 and 1. Only the two inbound synchronous IPC routes are needed. From the repository root on the integrated IPC-006 sources:

~~~sh
sh sdk/native-aarch64/build.sh --local-socket examples/native-local-sockets/server.c telemetry-server.elf
sh sdk/native-aarch64/build-rust.sh --local-socket examples/native-local-sockets/client/Cargo.toml telemetry-client.elf
sh sdk/native-aarch64/build-rust.sh services/socket/Cargo.toml socket-broker.elf
package() {
    kind=$1
    policy=$2
    shift 2
    python3 tools/build_native_el0.py --policy "$policy" --out "local-$kind-$policy" --elf telemetry-server.elf --arg "$kind" --priority 7 --peer-elf telemetry-client.elf --peer-arg "$kind" --peer-priority 4 --peer-elf socket-broker.elf --peer2-arg 3 --peer-priority 1 --socket-service 2 --ipc-route 0:2 --ipc-route 1:2 "$@"
}
for kind in stream dgram seq; do
    package "$kind" fifo
    package "$kind" rr --quantum-ticks 625000
done
~~~

The server binds /telemetry and /gate, listens for connected modes, and issues a blocking receive on /gate. The client sends its actual data, observes the configured pressure case and sends a one-byte completion marker on /gate. The server can then accept or read. This protocol establishes write-before-read ordering independently of interrupt frequency. Both connected modes use a write half-close while preserving the reverse acknowledgment direction. The client waits for the server's acknowledgment and final EOF. Each client closes its own descriptors. The expected exits, in slot order, are 73, 74 and 0.

| Mode | User-visible observations and independently expected product |
| --- | --- |
| stream | Send bytes 1..7 and request another 64 bytes starting at 8. The second write commits 57, and a further nonblocking write returns -11. Read 3, 16, then 45 bytes. The middle read crosses the original write boundary. Verify every byte 1..64, sum 2080, then EOF and reverse acknowledgment 32. |
| dgram | Bind the client at /origin. Send [10,11,12,13,14] and [20,21]. The server uses a three-byte buffer and MSG_TRUNC, observing original lengths 5 and 2, payloads [10,11,12] and [20,21], and /origin on both. No third record is readable. The copied sum is 74. Send acknowledgment 74 directly back to the reported address; the client verifies its source /telemetry. |
| seq | Connect before accept. Commit four three-byte records [i,10+i,20+i] for i=1..4. A fifth nonblocking record returns -11. Four eight-byte reads each return exactly three bytes in order, then EOF. Their sum is 150 and the acknowledgment is 150. |

Read socket_server_result as 12 little-endian u64 words through slot 0's own root. Indices 0/1 are the socket kind and matching gate marker (1/2/5); 2/3 are the sum and copied count (2080/64, 74/5, 150/12); 4..7 contain the four observed read lengths (unused entries zero); 8 is 10 for the datagram sender address including its family and terminator, otherwise zero; 9 is the acknowledgment byte; 10 is 1 after successful closes. Slot 1's socket_client_result is 12 little-endian signed 64-bit words: 0 kind, 1..4 send lengths, 5 pressure result (-11 for stream and seq, zero for dgram), 6 actual acknowledgment, 7 datagram acknowledgment source path length (10), 8 completed closes and 11 error. Both arrays have zero-initialized unused fields. Retain the actual first results for each package. These local AF_UNIX workloads exercise transport inside the guest; IPC-006's IPv4 TCP/UDP interface and independent peer traffic require their separate workload.
