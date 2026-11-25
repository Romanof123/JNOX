# AF_UNIX commit and backlog release review / AF_UNIX 연산 확정 및 backlog 릴리스 검토

**PASS on the sampled single-core AArch64 QEMU virt FIFO and RR configurations.** In both deadline orderings the receiving application saw the expected byte exactly once. An expired send on a full queue contributed zero records. A successful send contributed one whole record. A third blocking connect resumed on its original token after the first accept freed a place. Nine workloads ran under each policy, with an original boot and a separate register-traced boot for each position. Release independently decoded those 36 completed captures offline. [Acceptance criteria #381](https://github.com/Romanof123/JNOX/issues/381) · [Public first report and capture](https://github.com/Romanof123/JNOX/blob/090bec1015156a4fd63fcc10589eae0a3c393a44/docs/evidence/native-local-socket-commit-first.en-ko.md)

**단일 코어 AArch64 QEMU virt에서 관찰한 FIFO·RR 사례는 통과했다.** 먼저 만료된 수신은 나중에 도착한 바이트를 남겨 두었고, 먼저 준비된 수신은 그 바이트를 한 번 가져갔다. 가득 찬 큐에서 만료된 송신은 레코드를 추가하지 않았고, 기한 전에 성공한 송신은 레코드 하나를 추가했다. 세 번째 차단 연결은 첫 accept가 자리를 비운 뒤 원래 토큰으로 완료됐다. 배포 검토에서는 워크로드 9종을 정책별로 실행한 원본 18회와 별도 레지스터 추적 18회를 오프라인에서 독립 해독했다.

## What the applications and broker observed / 응용프로그램과 브로커의 관측 결과

| Workload / 사례 | Private result and matching register order / 전용 RAM 및 레지스터 결과 |
|---|---|
| Receive, timeout first / 수신 만료 우선 | Owner returned ETIMEDOUT (−110). Peer send 14 published one byte, 0x6d. The retry of receive token 8 prepared a count of zero in bank 1. Its reply of one byte returned ESTALE (−116). The next operation still selected bank 0. A fresh receive read exactly 0x6d, followed by EAGAIN. / 호출자는 −110으로 복귀했다. 송신 14 직후 토큰 8이 시험용 뱅크에서 레코드를 꺼냈지만 응답은 −116이었고 활성 뱅크는 0이었다. 새 수신에서 0x6d 한 바이트를 읽고 다음 수신에서 EAGAIN을 받았다. |
| Receive, ready first / 수신 준비 우선 | The same token 8 returned one byte before deadline. The matching broker reply succeeded, then bank 1 became live. META returned 0x0101, READ returned 0x6d and the next nonblocking receive returned EAGAIN. / 같은 토큰이 기한 전에 한 바이트를 반환했다. 응답 성공 뒤 뱅크 1이 활성화됐다. 메타데이터의 복사 길이와 원래 길이는 모두 1, 데이터는 0x6d였고 다음 수신은 EAGAIN이었다. |
| Full datagram queue, timeout first / 가득 찬 큐에서 송신 만료 | Four one-byte sends queued 0x11, 0x22, 0x33, 0x44. The receiver's token 26 removed the first record. The retry of send 24 prepared a fourth record in bank 1. Reply returned −116 and the next operation used bank 0. The receiver read the original four markers, got EAGAIN, then read the fresh record 0x7b. The 0xee candidate never appeared. / 토큰 26이 첫 레코드를 꺼낸 뒤 토큰 24가 시험용 뱅크에 0xee를 추가했지만 응답은 −116이었다. 수신 순서는 원래 네 바이트, EAGAIN, 후속 0x7b였다. |
| Full datagram queue, space first / 가득 찬 큐에서 공간 확보 우선 | The same peer drain released token 24. Its reply returned success with count 1, then selected bank 1. Reads yielded 0x11, 0x22, 0x33, 0x44, 0xee, EAGAIN and later 0x7b. / 같은 수신으로 공간이 생겼고 토큰 24의 한 바이트 응답이 성공한 뒤 뱅크가 바뀌었다. 0xee는 네 원래 레코드 뒤에 한 번만 도착했다. |
| Blocking backlog / 차단 연결 | The first two connects advanced occupancy to 1 and 2. Third connect token 29 stayed at 2/2 across retries. Accept token 31 changed it to 1/2, then token 29 appended once, replied successfully and restored occupancy to 2/2. Three accepts drained markers 0x33, 0x34, 0x35 in order. / 세 번째 연결은 2/2에서 대기하다가 첫 accept가 만든 한 자리에 한 번 들어갔다. 연결 세 개에서 수신한 표식도 원래 순서였다. |
| Peer death / 피어 종료 | A full sequenced-packet writer 46 returned 1 after receive 48. The next blocked writer 51 returned EPIPE (−32) after slot 1 exited with two open FDs, unread records and an unfinished bind. Slot 0 read EOF and rebound /origin. / 첫 차단 송신은 수신 48 뒤 성공했다. 두 번째 차단 송신은 피어 종료 뒤 EPIPE가 됐다. 생존 프로세스는 EOF를 읽고 피어가 쓰던 경로에 다시 바인딩했다. |

The deadline owner issued no ABORT. Between each retained finite call and its broker reply, the only intervening broker requests came from the other client. In the full-queue cases the receiver had also observed EAGAIN before the sender's next broker request. The fixtures used direct endpoint calls for synchronization. On the 18 traced boots the decoder paired 1,066 broker requests with 1,066 reply syscalls, checked all 326 successful COMMIT publications after their reply return and found four ESTALE replies. Direct synchronization calls account for the remaining kernel tokens. The JSON gives a zero-based register-record index for each distinguished event.

기한이 있는 호출자는 ABORT를 보내지 않았다. 보류된 유한 호출부터 브로커 응답까지 다른 클라이언트만 브로커에 새 요청을 보냈다. 가득 찬 큐 시험에서는 수신자가 EAGAIN까지 확인한 뒤에야 송신자가 새 브로커 요청을 보냈다. 동기화에는 두 응용프로그램 사이의 직접 IPC를 사용했다. 추적 18회에서 브로커 요청 1,066건과 응답 시스템 호출 1,066건을 짝지었다. 성공한 COMMIT 326건에서 응답 복귀 뒤 활성 뱅크 변경을 확인했고, 기한 만료로 거절된 응답은 4건이었다.

## Guest time / 게스트 시각

The fixtures requested wakeups relative to an absolute guest deadline. Offsets below are measured guest CLOCK_MONOTONIC samples from the application RAM, in milliseconds relative to that deadline. Each range covers four boots across both policies. A negative offset means before deadline.

아래 값은 원본과 추적 실행의 응용프로그램 RAM에 남은 게스트 단조 시각이다. 각 범위는 두 정책을 합한 4회의 관측이며, 음수는 기한 전을 뜻한다. 첫 번째 숫자 열은 프로그램이 요청한 깨움 시점이다.

| Case / 사례 | Requested peer wake / 요청 (ms) | Observed peer start / 피어 시작 (ms) | Owner return / 호출자 복귀 (ms) |
|---|---:|---:|---:|
| Receive expired / 수신 만료 | +50 | +50.272 to +50.419 | +0.603 to +0.811 |
| Receive ready / 수신 준비 | −100 | −99.535 to −99.278 | −98.413 to −95.212 |
| Send expired / 송신 만료 | +65 | +65.255 to +65.439 | +0.710 to +0.956 |
| Send ready / 송신 준비 | −150 | −149.443 to −149.172 | −148.817 to −146.964 |

The third blocking connect lasted 98.072–98.515 guest ms. The first accept began 87.516–96.797 ms after that connect started. The connect returned 1.282–10.962 ms after entry to accept. Register order establishes where accept actually freed space. The measurements cover finite samples with the recorded priority configuration. Universality across simultaneous events, scheduler interleavings or hardware timing remains unmeasured.

세 번째 연결은 게스트 시각으로 98.072–98.515 ms 동안 차단됐다. 첫 accept 진입은 연결 시작 후 87.516–96.797 ms, 연결 복귀는 accept 진입 후 1.282–10.962 ms였다. 실제 큐에서 자리가 생긴 순서는 레지스터로 확인했다. 동일 시각 경합, 가능한 모든 스케줄 순서, 실기기 시간은 이 측정 범위를 벗어난다.

## Controls and cleanup / 일반 동작 및 종료 정리

| Control / 대조 사례 | FIFO and RR results / 두 정책의 결과 |
|---|---|
| Stream / 바이트 스트림 | Writes 7 + 57 filled 64 bytes. The next write got EAGAIN. Reads 3 + 16 + 45 checked consecutive bytes 1–64, sum 2,080, ACK 32, then EOF. / 분할 송신 경계를 넘는 읽기에서 64바이트의 값과 순서, EOF를 확인했다. |
| Datagram / 데이터그램 | Sent records of lengths 5 and 2. MSG_TRUNC returned 5 and 2 when copying 3 and 2 bytes. /origin and /telemetry source name checks passed, ACK 74. / 길이, 절단된 데이터, 양쪽의 발신 경로, 확인 바이트를 확인했다. |
| Sequenced packets / 순서 보장 패킷 | Four ordered three-byte records contained [i, i+10, i+20] for i=1…4. The fifth nonblocking send got EAGAIN. ACK 150 and EOF followed. / 세 바이트씩 네 레코드의 경계와 내용을 검증했고, 가득 찬 큐에서의 반환값과 EOF를 확인했다. |
| Process exits / 종료값 | C owner, peer, broker respectively returned 73/74/0 for ordinary transports, 81/82/0 for finite receive and backlog, 201/202/0 for full sends and 62/63/0 for death. Peer-death uses two C applications. / 일반 전송·기한 경합·피어 종료의 기대 종료값이 원본과 추적에서 일치했다. |

All 36 RAM captures resolved the three private address spaces independently. The broker's saved main frame identified the actual live selector, which also matched the final bank register in each traced boot. All eight live socket objects were free. Counts of bound objects, queued connections, bytes and records were zero, as were the 12 FD entries. All three clients' request, response and pending-token options were empty. The three kernel endpoints were dead and empty with no pending exit notification. Each broker has two private 4,352-byte socket banks, 816 bytes of client slots, and a 272-byte client trial value in the frozen code. The inactive bank may retain old values. The fixed UART journal reported 50–166 dropped rows on 28 boots and zero on eight. Application result vectors and the separately captured broker register records provide the order witnesses above.

36회 모두 개별 주소 공간의 페이지 테이블을 따라 실제 활성 뱅크를 찾았고, 추적 18회에서는 마지막 뱅크 레지스터와도 일치했다. 소켓 객체 8개, 바인딩, 대기 연결, 큐의 바이트·레코드 수 및 FD 12개는 모두 비어 있었다. 각 클라이언트의 조립 중 요청·응답·보류 토큰, 커널 엔드포인트 3개의 요청·수신 대기·완료·종료 통지도 비어 있었다. 브로커는 전용 4,352바이트 뱅크 두 개 및 클라이언트 상태 816바이트를 사용하고, 한 번에 272바이트의 클라이언트 사본으로 연산을 준비한다. 비활성 뱅크에는 이전 값이 남을 수 있다. UART 스케줄 기록은 28회에서 각 50–166행이 누락됐고 나머지 8회는 누락이 없었다.

## Provenance and replay / 출처와 재계산

Tested program `9912ae3db95aec3f1a67f690d01c7379339168d1`, tree `fe8da16304f4c97b35533c35f897cab6cd048836`, broker ELF SHA-256 `2ba00646fd51b297dcc762fb4ff81df70036542376ffe4ddd32ae16cd9b4bdc5`. The first packet is the public Git blob `5d5d14e48e64bc956a5335755c9038e8d197234a`. Public evidence commit `090bec1015156a4fd63fcc10589eae0a3c393a44` carries the same 239 paths and Git blobs across arch, boards, kernel, native AArch64 SDK, socket service, local examples and the three build/capture scripts compared. The guest revision remains the one recorded in the launch manifests. One initial full-queue C fixture build failed on an unavailable int64_t typedef. The retained correction uses the AArch64 signed long type, and all reported full-queue boots used that successfully compiled fixture. [First archive](https://github.com/Romanof123/JNOX/blob/090bec1015156a4fd63fcc10589eae0a3c393a44/docs/evidence/native-local-socket-commit-first.tar.gz)

게스트의 소스 및 ELF는 패키지 매니페스트, 세션 해시, UART가 기록한 ELF ID로 대조했다. 공개 자료 커밋과의 비교에서는 위 범위의 239개 경로와 Git 블롭이 일치했다. full-queue C 시험의 최초 컴파일 실패 기록도 첫 보관물에 있다. 실제 게스트는 AArch64의 부호 있는 long을 사용한 수정본을 실행했다.

The earlier AF_UNIX release retains its original limits, including its explicit-ABORT case and nonblocking backlog retry. This report adds independent observations for the two finite orderings and blocking admission. The wider IPC-006 decision also depends on the evidence tracked in #376 and #24. / 기존 AF_UNIX 승인은 명시적 ABORT와 비차단 재시도가 입증한 범위를 유지한다. 이번 자료가 기한 전후의 실제 순서와 차단 연결을 보완한다. IPC-006 전체 판정에는 #376과 #24의 다른 증거도 필요하다. [Prior limited AF_UNIX release](https://github.com/Romanof123/JNOX/blob/31de1c8ca01f1baf8ad6b792964080123de094d8/docs/evidence/native-local-socket-release.en-ko.md) · [IPC-006 acceptance #376](https://github.com/Romanof123/JNOX/issues/376) · [Parent acceptance #24](https://github.com/Romanof123/JNOX/issues/24)

Extract the release supplement and place the first capture archive beside it. Run the following with Python 3.9+ and its standard library. The decoder validates the pinned original archive SHA-256 `9819033ffec7ee0d1867f5348bb20b76cc37dac6c0284bccb5cb7fbe2dd96165`, all session hashes, loaded ELF identities, page-table walks, bank selectors and outcomes. It emits deterministic UTF-8 JSON. / 배포 보관물을 풀고 첫 보관물을 같은 폴더에 놓은 뒤 다음 명령을 실행하면 원시 자료에서 같은 결과가 복원된다.

```sh
python3 decode.py native-local-socket-commit-first.tar.gz -o recomputed.json
python3 -c "from pathlib import Path; assert Path('recomputed.json').read_bytes() == Path('findings.json').read_bytes()"
```
