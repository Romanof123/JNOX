# Native local socket RPC v1

Package the broker as an ordinary process. Its only argument after argv[0] is a single ASCII digit containing the bitmask of authorized client slots. For clients 0 and 1 use `3`. The broker receives requests and exit events from SVC 37. It uses SVC 23 for ordinary replies and SVCs 184–186 for descriptor transfer with the optional `--shared-fd` package feature. The kernel supplies the client identity. Each client has private input, pending-call and output state. SDK operations use an indefinite CALL deadline.

Every request and reply is one u64. Byte 0 is the least significant byte. Replies encode errors as sign-extended negative errno. Success values and chunks fit a nonnegative i64.

| Tag (byte 7) | Bytes 6 through 0 | Success |
| --- | --- | --- |
| 0x11 BEGIN | opcode, fd, arg, pathname length, payload length, flags, zero | 0 |
| 0x12 CHUNK | sequential byte offset in byte 6, up to six input bytes starting at byte 0 | 0 |
| 0x13 COMMIT | zero | descriptor, count or 0. Operation 16 also returns the installed FD in byte 1, zero for none. |
| 0x14 META | zero | copied length in byte 0, original length in byte 1, source pathname length in byte 2. Operation 16 adds the installed FD in byte 3 and the control-truncated flag in bit 32. |
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
| 15 | send one backing FD on a stream | sender-local backing FD, 96–103 | 1–64 data bytes |
| 16 | receive stream data with optional control | data capacity, 0–64 | none |

All operations other than socket address the current caller's descriptor. Paths occupy at most 31 bytes. A send transaction stages at most 64 data bytes. Record sends have an additional 32-byte limit and commit one complete record. Flags bit 0 requests nonblocking behavior. Operation 16 also accepts bit 1 when the caller has room for one FD. The remaining bits are reserved. Blocking connect, accept, send and receive retain the COMMIT token when the service reports EAGAIN. A state change or authenticated client exit prompts another bounded pass over pending operations. Stream partial success returns its actual byte count immediately.

## Stream descriptor transfer

Operation 15 attaches a backing reference to the first committed byte of a stream write. A partial send commits the reference once. Subsequent sends of the remaining bytes use operation 6. Up to eight references can wait across all receiving endpoints. Both operations 7 and 16 stop before the next byte carrying a reference. A zero-capacity receive leaves it queued. When the first byte read carries a reference, operation 16 installs it into the receiver's own FD table if the caller supplied space. With insufficient control space, it returns the bytes and sets the control-truncated flag. Operation 7 drops the accompanying reference and retains its original byte-only reply format.

The broker passes the authenticated send COMMIT token and sender-local FD to HOLD (SVC 184). EL1 validates the sender, retains its backing reference, and accepts the reply atomically. The send token returned to the broker identifies that escrow. The receive COMMIT uses INSTALL (SVC 185) with its own token and the escrow selected from the receiving byte queue. EL1 installs the FD for that caller and replies with the byte count and installed FD. The broker mirrors that FD in META so the SDK can verify it and close it on a subsequent output error. An expired COMMIT or a full receiver FD table preserves the live queue. An unavailable HOLD fails the send without publishing data.

Each queued reference belongs to the receiving endpoint, including a pending connection before accept. Closing the sender leaves queued bytes and their references available to the receiver. Read shutdown, receiver close, listener close with pending connections, authenticated client exit and control truncation discard the corresponding escrows through SVC 186. The broker discards live escrows on a controlled service exit. EL1 reclaims the broker's escrow table on broker death.

A raw finite-deadline COMMIT takes effect when the kernel accepts its one-shot
reply. The broker copies its live socket table into a second process-private
static table, tries the operation there, and selects that table only after a
successful operation and accepted reply. The kernel arbitrates expiry at the
reply SVC, including expiry during preemption. A stale token discards the
trial and ends the staged COMMIT. No other broker operation can run between
reply acceptance and table selection. BEGIN, CHUNK and response framing
likewise publish their per-client updates only on reply acceptance.

Each COMMIT attempt, including a blocked retry, copies one fixed socket table.
The two tables occupy static storage. Each client retains at most one blocked
token and its bounded request. A subsequent authenticated call from that
client cancels its old pending slot; an authenticated death clears its staging,
closes its live descriptors and prompts retries. Infinite-deadline SDK calls
use the same protocol. The earlier 32 captures describe the earlier binary.

RECV_RIGHTS BEGIN flags bit 2 requests MSG_CMSG_CLOEXEC. INSTALL carries this flag in bit 8 of its broker-local count word. EL1 validates the live COMMIT and receiver slot, installs the right and its descriptor-local flag together, and completes the reply. Plain receives and truncated control buffers do not install a descriptor.

Socket operations 17 (local name) and 18 (peer name) take the caller’s socket slot with zero argument, path and payload. The broker looks up that slot in the authenticated caller’s four-entry table. COMMIT returns zero and stages up to 31 pathname bytes in the existing META path and READ response. An unnamed endpoint has a zero-byte path. Queries leave data queues, EOF and transferred rights intact. Each accepted endpoint carries its listener’s name. A disconnected association retains the peer’s final name before its internal object slot is reused. A newly created unconnected socket returns ENOTCONN for a peer query.

동작 17과 18은 호출자의 소켓 슬롯으로 각각 자기 주소와 상대 주소를 조회한다. 기존 META/READ 규약으로 최대 31바이트 경로를 전달한다. 익명 종단의 경로 길이는 0이며, 해제된 상대의 내부 슬롯이 재사용되어도 기존 연결은 마지막 상대 이름을 유지한다.
