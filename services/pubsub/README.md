# Native publish/subscribe broker

This ordinary Rust process owns four static topics, three subscription slots per client and four records of up to 32 bytes per subscription. It runs in slot 3 on the four-process fixed FIFO/RR profile. The kernel routes clients 0, 1 and 2 to the broker over synchronous IPC. SVC 52 blocks until a request or a client exit arrives. It returns status, one-shot reply token, request word and authenticated client slot in x0–x3. An exit has token zero. SVC 23 completes a request with one reply word.

The packager supplies two private argv strings to the broker. Each contains exactly three lowercase hexadecimal digits in client order. The first string holds publish masks and the second holds subscribe masks. Bit t grants access to topic t for that client. The service installs these grants once before receiving any requests. For a publisher on slot 0 and subscribers on slots 1 and 2, all on topic 0, the strings are `100` and `011`. All three clients have explicit routes to slot 3, including a client with zero topic grants.

```sh
sh sdk/native-aarch64/build-rust.sh services/pubsub/Cargo.toml pubsub.elf
```

## RPC v1

Each CALL carries one u64 word. Byte 0 is least significant. An error reply is a sign-extended negative errno. Each process can have one in-progress transaction and one completed response. All chunk data are little-endian.

| Frame | Contents | Successful reply |
| --- | --- | --- |
| BEGIN 0x31 in byte 7 | Bytes 0–3 contain exact input size, topic, opcode and flags. Bytes 4–6 are zero. | 0 |
| CHUNK 0x32 | Byte 6 contains the exact next input offset. Bytes 0–5 carry up to six input bytes. Zero-pad the final chunk. | 0 |
| COMMIT 0x33 | Bytes 0–6 are zero. All declared input must have arrived. | Response byte count |
| READ 0x34 | Byte 6 contains a response offset. Bytes 0–5 are zero. | Up to six output bytes, starting in byte 0, zero-padded |
| ABORT 0x35 | Bytes 0–6 are zero. | 0 |

The opcode defines all bytes in the staged input and output. A handle consists of two complete little-endian u64 values, global slot index followed by generation. Generation and publication counters are full width. Topic is zero for handle operations. Only RECEIVE accepts flag bit 0 (WAIT). Other flag bits are reserved.

| Opcode | Input | Output |
| --- | --- | --- |
| 1 SUBSCRIBE | Topic in BEGIN, empty body | 16-byte handle |
| 2 UNSUBSCRIBE | Handle | Empty |
| 3 PUBLISH | Topic in BEGIN, 0–32 data bytes | Assigned topic sequence, u64 |
| 4 OBSERVE | Handle | Queued count u8, high-water count u8, loss count u64, front sequence u64. Front sequence is zero when empty. |
| 5 RECEIVE | Handle | Sequence u64, length u8, then that many sample bytes |
| 6 PEEK | Handle | Same response as RECEIVE, with the sample left queued |

A successful BEGIN clears the previous readback. A second BEGIN while input is staged returns EBUSY. An out-of-order CHUNK, nonzero padding, invalid reserved fields or an incomplete COMMIT returns EINVAL. A complete COMMIT consumes the staged request, including one rejected for rights or a stale handle. An expired token discards the staged operation. READ can retrieve the first through the last actual response byte, including a partial last chunk. ABORT clears private framing state.

Publishing assigns sequence 1 to the first sample of each topic. Delivery visits subscriptions in registration order. A full queue drops its newest incoming sample and increments its own loss count. Other subscribers can still receive the same publication. Unsubscribe clears the queue; reusing that slot gets a fresh generation. An unauthorized topic operation returns EACCES, a different client's handle returns EPERM, an expired generation returns ESTALE and exceeding the slot quota returns ENOSPC. Counter overflow returns EOVERFLOW before delivery changes.

Empty nonblocking RECEIVE and PEEK return EAGAIN. Empty WAIT retains one reply token per client while an authorized publisher for that topic is alive. A successful publication or authenticated process exit scans at most three held receives. After the last possible publisher exits, committed samples can still be drained. WAIT then returns EPIPE. A departing subscriber's queued records are reclaimed. It contributes no new full-queue losses.

The service prepares each mutation in a second, statically allocated state buffer. A successful kernel reply selects that buffer before the single-threaded broker serves another request. If the token has expired, the active sequence counters, subscriptions, queues and loss totals retain their previous values. A newly arrived call from a slot with an old pending receive establishes that the kernel already canceled the older call.

## Private RAM observation

The symbol `pubsub_report` is an array of fifteen u64 values stored after dispatch and wake processing. The fields in order are magic `0x4a4e505338`, live-client mask, active subscriptions, queued records, peak subscriptions, peak queued records, lifetime drops, last sequence for topics 0–3 (four words), rejected stale replies, currently held waits, peak simultaneous waits, and accepted mutating COMMIT/reception count. This private observation remains in the guest image when the broker exits after all three authenticated client deaths. Broker exit 0 records normal completion. Exit 81 identifies invalid startup configuration, 82 a broker receive failure, 83 invalid event identity and 84 a reply transport failure.
