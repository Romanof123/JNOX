# Native local socket RPC v1

Package the broker as an ordinary process. Its only argument after argv[0] is a single ASCII digit containing the bitmask of authorized client slots. For clients 0 and 1 use `3`. The broker receives requests and exit events from SVC 37 and replies using SVC 23. The kernel supplies the client identity. Each client has private input, pending-call and output state. SDK operations use an indefinite CALL deadline.

Every request and reply is one u64. Byte 0 is the least significant byte. Replies encode errors as sign-extended negative errno. Success values and chunks fit a nonnegative i64.

| Tag (byte 7) | Bytes 6 through 0 | Success |
| --- | --- | --- |
| 0x11 BEGIN | opcode, fd, arg, pathname length, payload length, flags, zero | 0 |
| 0x12 CHUNK | sequential byte offset in byte 6, up to six input bytes starting at byte 0 | 0 |
| 0x13 COMMIT | zero | descriptor, count or 0 |
| 0x14 META | zero | copied length in byte 0, original length in byte 1, source pathname length in byte 2 |
| 0x15 READ | offset in byte 6, otherwise zero | up to six output bytes starting at byte 0 |
| 0x16 ABORT | zero | 0 |

BEGIN opens a transaction. A second BEGIN during input returns EBUSY. CHUNK covers the pathname followed by the payload, without a terminator. The offset must equal the number of bytes already staged. Unused final bytes must be zero. COMMIT requires every declared byte. A successful receive retains its copied payload followed by the source path for META and READ. READ rejects offsets at or beyond the end. Successful BEGIN or ABORT clears the prior response.

| Opcode | Operation | arg | Input |
| --- | --- | --- | --- |
| 1 | socket | 1 stream, 2 datagram, 5 sequenced packet | fd zero |
| 2 | bind | zero | pathname |
| 3 | connect | zero | pathname |
| 4 | listen | backlog | none |
| 5 | accept | zero | none |
| 6 | send | zero | optional destination pathname, then data |
| 7 | receive | capacity, 0–64 | none |
| 8 | shutdown | 0 read, 1 write, 2 both | none |
| 9 | close | zero | none |

All operations other than socket address the current caller's descriptor. Paths occupy at most 31 bytes. A send transaction stages at most 64 data bytes. Record sends have an additional 32-byte limit and commit one complete record. Flags bit 0 requests nonblocking behavior. All other bits are reserved. Blocking connect, accept, send and receive retain the COMMIT token when the service reports EAGAIN. A state change or authenticated client exit prompts another bounded pass over pending operations. Stream partial success returns its actual byte count immediately.
