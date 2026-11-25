# Local socket client

Build a native C or Rust client with the opt-in linker configuration. A Rust application enables the local-socket feature of its jnox-native dependency. Both interfaces use the same C encoder and synchronous IPC transport. Place the ordinary broker in the chosen slot and grant a directed IPC route from every client to that broker.

~~~sh
sdk/native-aarch64/build.sh --local-socket client.c client.elf
sdk/native-aarch64/build-rust.sh --local-socket client/Cargo.toml client-rust.elf
~~~

C clients include sys/socket.h and sys/un.h. Call jnox_socket_broker(broker_slot) before socket(AF_UNIX, type, 0). Rust clients call jnox_native::local_socket::set_broker(slot). The broker selection is fixed for the process's lifetime. Socket descriptors 64 through 67 identify that client's four broker slots. The optional linker wrappers route read, write and close for socket descriptors and delegate other descriptors to the original native SDK. SDK builds without --local-socket keep the ordinary link path.

A pathname contains 1 to 31 bytes and uses sockaddr_un in C or a byte slice in Rust. Lazy peer address lookup on accept is unavailable in wire v1. Use accept(fd, NULL, NULL) or Rust accept(fd); requesting a C peer address on accept fails before consuming a connection. recvfrom supplies a datagram source address when present. MSG_TRUNC requests the original record length, which may exceed the destination slice. Streams transfer up to 64 bytes per call, and record sends over 32 bytes fail with EMSGSIZE. SOCK_NONBLOCK applies to the created descriptor; MSG_DONTWAIT applies to a single transfer. A newly accepted descriptor defaults to blocking.

Each BEGIN frame names the operation, broker descriptor, argument and exact byte counts. Six-byte little-endian CHUNK frames carry path then data. COMMIT returns the operation result, with META and READ retrieving a receive's payload and source pathname. A per-process gate prevents interleaved frame sequences, including the receive readback. A concurrent call gets EBUSY. A failed chunk is followed by ABORT. No pointer is sent to the broker. C returns -1 with positive errno on failure; Rust returns the negative error in Result.
