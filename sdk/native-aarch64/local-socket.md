# Local socket client

Build a native C or Rust client with the opt-in linker configuration. A Rust application enables the local-socket feature of its jnox-native dependency. Both interfaces use the same C encoder and synchronous IPC transport. Place the ordinary broker in the chosen slot and grant a directed IPC route from every client to that broker.

~~~sh
sdk/native-aarch64/build.sh --local-socket client.c client.elf
sdk/native-aarch64/build-rust.sh --local-socket client/Cargo.toml client-rust.elf
~~~

C clients include sys/socket.h and sys/un.h. Call jnox_socket_broker(broker_slot) before socket(AF_UNIX, type, 0). Rust clients call jnox_native::local_socket::set_broker(slot). The broker selection is fixed for the process's lifetime. Socket descriptors 64 through 67 identify that client's four broker slots. The optional linker wrappers recognize socket descriptors and shared backing descriptors, and delegate other descriptors to the original native SDK. SDK builds without --local-socket keep the ordinary link path.

A pathname contains 1 to 31 bytes and uses sockaddr_un in C or a byte slice in Rust. Lazy peer address lookup on accept is unavailable in wire v1. Use accept(fd, NULL, NULL) or Rust accept(fd); requesting a C peer address on accept fails before consuming a connection. recvfrom supplies a datagram source address when present. MSG_TRUNC requests the original record length, which may exceed the destination slice. Streams transfer up to 64 bytes per call, and record sends over 32 bytes fail with EMSGSIZE. SOCK_NONBLOCK applies to the created descriptor; MSG_DONTWAIT applies to a single transfer. A newly accepted descriptor defaults to blocking.

Each BEGIN frame names the operation, broker descriptor, argument and exact byte counts. Six-byte little-endian CHUNK frames carry path then data. COMMIT returns the operation result, with META and READ retrieving a receive's payload and source pathname. A per-process gate prevents interleaved frame sequences, including the receive readback. A concurrent call gets EBUSY. A failed chunk is followed by ABORT. No pointer is sent to the broker. C returns -1 with positive errno on failure; Rust returns the negative error in Result.

## Shared backing FDs on AF_UNIX streams

For a native virt FIFO or RR package, select `--shared-fd` with the AF_UNIX socket broker. The client needs a directed route to the broker. The C declarations are in `<jnox/shm.h>` and `<sys/socket.h>`. Rust uses the existing `jnox_native::local_socket` module and the same C message implementation.

`jnox_shm_create(size)` creates a zeroed object of 1 through 307200 bytes. `jnox_shm_size(fd)` returns its exact byte length. There are four objects system-wide, eight in-flight references, eight backing FDs per process (96 through 103) and four live mappings per process. `jnox_shm_map(fd, JNOX_SHM_READ)` creates a read-only mapping. Add `JNOX_SHM_WRITE` for a writable mapping of a writable FD. Mapping failure returns `JNOX_SHM_MAP_FAILED` and sets errno. `jnox_shm_dup_readonly(fd)` creates another FD for the same backing that permits only read-only mappings. Transfer preserves that restriction.

Use the pointer within the object's exact byte length and coordinate conflicting access with other processes. `jnox_shm_unmap(base)` releases one mapping. `close(fd)` or `jnox_shm_close(fd)` releases one descriptor. The mapping holds its own reference and remains usable after descriptor close. A successful ancillary send also holds a reference, so the sender can immediately close or reuse its descriptor number. The receiver owns the descriptor number reported in its own process and is responsible for closing it. Read and write through a live backing FD return ENOTSUP. Use the mapping for byte access.

Both message calls take a connected AF_UNIX stream, a null address and zero address length. Up to eight `iovec` entries are accepted, including empty spans. Only the first 64 bytes can transfer per call. A short positive `sendmsg` has already sent the control with its first byte. Retry only the unsent bytes. A control-bearing send requires at least one payload byte. Flags may be zero or `MSG_DONTWAIT`. A nonblocking socket keeps its existing behavior.

For one backing descriptor, use an aligned control buffer, set `cmsg_level=SOL_SOCKET`, `cmsg_type=SCM_RIGHTS`, and `cmsg_len=CMSG_LEN(sizeof(int))`. A send accepts control lengths from `CMSG_LEN(sizeof(int))` through `CMSG_SPACE(sizeof(int))`. A receive needs at least `CMSG_LEN(sizeof(int))` to install the FD. With a smaller or absent control buffer, delivery of the associated byte discards its right and sets `MSG_CTRUNC`. A plain `read` or `recv` consumes the corresponding byte and discards its right. A zero-byte receive does not consume a right. A byte-only send sets `msg_controllen` to zero. The returned control length is zero when no FD was delivered.

~~~c
union { struct cmsghdr align; unsigned char bytes[CMSG_SPACE(sizeof(int))]; } control;
char tag = 'B';
struct iovec vec = { &tag, 1 };
struct msghdr msg = { .msg_iov = &vec, .msg_iovlen = 1,
                      .msg_control = control.bytes, .msg_controllen = sizeof(control.bytes) };
struct cmsghdr *c = CMSG_FIRSTHDR(&msg);
c->cmsg_level = SOL_SOCKET;
c->cmsg_type = SCM_RIGHTS;
c->cmsg_len = CMSG_LEN(sizeof(int));
__builtin_memcpy(CMSG_DATA(c), &backing_fd, sizeof(backing_fd));
ssize_t sent = sendmsg(socket_fd, &msg, 0);
~~~

For `recvmsg`, supply the same aligned buffer and reset `msg_controllen` to its capacity before each call. On success inspect `msg_flags` and `CMSG_FIRSTHDR`. The client validates the entire broker response before writing the payload or modifying message outputs. On a readback error after an FD was installed, the client closes that FD and returns the error. Bytes already consumed by the broker stay consumed.

Rust clients call `shm_create`, `shm_dup_readonly`, `shm_size`, `shm_map(fd, writable)` and `close`. A `ShmMapping` releases its mapping on drop or via `unmap(self)`. Its byte accessors require an unsafe block to document lifetime and cross-process synchronization. `sendmsg(fd, bytes, Some(backing_fd), flags)` attaches one right. `recvmsg(fd, dst, accept_fd, flags)` returns a `ReceivedMessage` with the byte count, optional local FD and control-truncation indicator.

Wire operation 15 carries a single sender-local FD in the argument byte. Operation 16 requests a receive, with control capacity indicated by BEGIN flag bit 1. Its COMMIT reply carries the byte count in bits 0–7 and the installed receiver-local FD in bits 8–15, or zero in that field when no FD was installed. META mirrors the FD in bits 24–31 and signals a discarded right in bit 32. Existing byte-only wire operations retain their original format.

## IPv4 broker

Use the same --local-socket SDK build option and select the configured --net-service broker with jnox_socket_broker or Rust set_broker. The process needs an IPC call route to that broker. Use AF_INET with SOCK_STREAM (TCP) or SOCK_DGRAM (UDP); protocol zero selects the corresponding default. C also accepts IPPROTO_TCP or IPPROTO_UDP for its matching kind. Include netinet/in.h for sockaddr_in and arpa/inet.h for htons, ntohs, htonl and ntohl. Set sin_family to AF_INET and store sin_port and sin_addr.s_addr in network order.

In Rust, Ipv4Endpoint takes four address octets in network order and a host-order u16 port. Use socket4, bind4, connect4, send_to4 and recv_from4. The latter returns a count and an optional source endpoint. TCP supplies no source address on receive; UDP supplies its source, including on truncation. MSG_TRUNC returns the full UDP length while copying only the bytes that fit. Each TCP transfer handles up to 64 bytes; UDP accepts one record of at most 32 bytes. SOCK_NONBLOCK, MSG_DONTWAIT, listen, accept, shutdown and close follow the descriptor's selected family. An accepted connection retains its listener's family and starts in blocking mode.

IPv4 uses wire operations 10 through 14 for socket, bind, connect, send, and receive. An endpoint is exactly six bytes: two big-endian port bytes followed by four address bytes. A connected send omits the destination. The family and sockaddr padding are local SDK metadata. Creating the socket records its family in the process descriptor table, allowing ordinary read, write, send, and recv to dispatch to the correct broker operation.
