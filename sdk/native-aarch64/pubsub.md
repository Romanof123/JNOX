# Native publish/subscribe SDK

C clients include jnox/pubsub.h. Rust clients enable the pubsub Cargo feature of jnox-native and import jnox_native::pubsub. Both use the same bounded v1 implementation. Build clients with the ordinary SDK and its opt-in linker argument.

~~~sh
sdk/native-aarch64/build.sh --pubsub app.c app.elf
sdk/native-aarch64/build-rust.sh --pubsub Cargo.toml rust-app.elf
~~~

Pass the configured broker slot explicitly to each call (3 in the four-process example). The kernel authenticates the client through its synchronous IPC route; authorization for the topic comes from broker configuration. Treat the slot/generation pair as an opaque handle. A slot of zero is valid.

| Operation | Success result |
|---|---|
| subscribe | Fresh subscription handle |
| unsubscribe | Releases that subscription |
| publish | Sequence number of an accepted sample, 0 to 32 payload bytes |
| observe | Current queue occupancy, high-water mark, cumulative lost count, front sequence (zero when empty) |
| peek | Copies the front sample without removing it |
| receive | Copies and consumes one sample, optionally waiting for a publication |

C prefixes the operation names with jnox_ and returns zero or negative errno; successful typed values are written through the final output pointer. Rust returns Result with the same negative errors. Samples carry their own byte length. Zero-length payloads are supported. Empty peek and nonblocking receive return -11. Waiting receive drains committed samples and returns -32 on an empty subscription once its last authorized publisher has exited.

A transaction sends BEGIN, zero-padded 6-byte chunks, COMMIT and bounded READ frames. The per-process SDK gate covers the whole sequence including a blocked COMMIT; overlapping SDK use returns -16. The client checks exact result sizes, sample length and final chunk padding before publishing any caller-visible output. An error during response readback is not a reason to retry an already committed publish or consuming receive. Disabled SDK builds omit the client object.
