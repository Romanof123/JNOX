# IPC-008 first independent verification / 발행·구독 독립 검증

Ten separately launched single-core AArch64 QEMU virt guests passed. Two uninstrumented runs used the documented C publisher, Rust active subscriber, C stalled subscriber and Rust broker. Two more ran the same kernel and ELFs with the corrected capture CLI. Four register-traced ordinary-client controls exercised finite deadlines, publication, owner exit and authorization. Two three-process controls exercised FIFO/RR with the service disabled.

단일 코어 AArch64 QEMU virt에서 별도로 실행한 게스트 10개를 확인했다. 기본 FIFO·RR는 C 발행자, Rust 활성 구독자, C 지연 구독자, Rust 브로커의 실제 ELF를 사용했다. 같은 이미지로 수정된 수집 도구를 각각 실행했다. 추가 게스트 네 개는 유한 대기 만료·성공 순서와 종료 처리를, 기존 3프로세스 게스트 두 개는 서비스 비활성화를 확인했다.

| Case / 사례 | FIFO | RR |
| --- | --- | --- |
| Named first guest / 기본 실행 | 71, 72, 73, 0 | 71, 72, 73, 0 |
| Retained / dropped fixed decision rows | 128 / 1044 | 128 / 1045 |
| Expiration before publish / 발행 전 만료 | wait token 26 returned −110 to reader, delivery reply to token 26 returned −116 | same |
| Ready before deadline / 기한 전 전달 | wait token 26 returned 10 bytes, delivery reply to token 26 returned 0 | same |
| Three-process legacy / 기존 3프로세스 | 7 × 21, zero missing | 7 × 21, zero missing |

The named publisher's IDs were 1–8. Byte lengths were 0, 1, 7, 32, 11, 3, 19 and 6. Offline replay compares the original bytes with the generator and each reader's private payload. The active reader had eight observations of one queued, high-water one, zero loss and the matching front ID. The stalled reader showed four queued, high-water four, exactly two lost and front ID 1. It read 1, 2, 3, 4, discarded its unread 7 by unsubscribing and received 8 under the replacement handle. The stale and empty statuses were −116 and −11. Independent package grants returned −13 on the publisher's subscribe and the stalled reader's publish. See the actual byte strings and raw results in `decoded.json`.

활성 구독자는 1~8번 표본 전부를 순서대로 받았고 손실은 0이었다. 지연 구독자의 대기·최고 수위는 4, 손실은 2였다. 1~4번을 읽은 후 7번을 남기고 해지했다. 슬롯 6의 세대가 1에서 2로 바뀌었고 재구독 직후 큐는 비어 있었다. 새 핸들은 8번을 받았다. 발행과 구독 각각에 대한 권한 거부는 −13이었다.

In the four deadline controls the broker received the slot-1 WAIT COMMIT with kernel token 26, then the publisher's COMMIT with token 31. The broker successfully replied to publication 31 before attempting to deliver to 26. Another RPC from client 1 could only proceed past its ordinary peer rendezvous after the publisher completed. The broker's authenticated RECEIVE trace independently records no intervening broker request from that client. In the expired order, the reader observed the sample still queued and subsequently read ID 1, payload A5, exactly once. In the ready order its ten response bytes decode to ID 1, length 1, payload A5, with the queue already empty. Both orders recorded zero loss. A subsequent held WAIT (59 expired-order, 54 ready-order) received −32 after authenticated publisher death. The publisher-death, pre-reply and accepted kernel-return instructions are preserved in the same register trace. The stalled client showed the same queued ID before and after the −13 refusal, then exited with a real unread sample. The active client observed −1 on both operations using the genuine foreign handle transferred by its peer.

기한 시험에서 브로커는 토큰 26에 대한 대기와 토큰 31의 발행을 순서대로 받았다. 발행 반영 후 만료된 26의 응답은 −116으로 거부되어 표본이 남았고, 반대 순서에서는 같은 토큰의 응답이 성공해 표본이 정확히 한 번 소비됐다. 발행 후 지연 구독자는 권한 거부 전후 동일한 큐를 관측하고 읽지 않은 표본을 남긴 채 종료했다. 두 번째 대기는 발행자 종료 후 −32를 받았다. 다른 구독자의 실제 핸들로 시도한 조회·해지는 모두 −1이었다.

The named broker reported two peak subscriptions, five peak records, two lifetime drops, topic-0 sequence 8, zero live clients, zero subscriptions, zero queued records and zero held waits at termination. Its peak held waits on the named runs was zero. The actual blocked wake is established by the separate deadline controls. Their broker counters were two peak subscriptions, two peak records, zero drops and one peak held wait. All four kernel endpoint slots were dead, with no receiving slot, request, pending completion or pending exit event. The replay uses an independently compiled AArch64 Rust DWARF layout for this exact source and checks its size against the packaged ENDPOINTS symbol.

기본 실행에서 브로커는 최고 구독 2개·보관 표본 5개·누적 손실 2개를 기록했다. 종료 후 클라이언트, 구독, 표본, 대기 수는 모두 0이었다. 기본 실행의 최고 보류 대기는 0이었다. 실제로 차단된 대기의 기상은 별도 기한 시험에서 관측했다. 커널 엔드포인트 네 개의 수신 대기·요청·완료·종료 알림도 모두 비었다.

For each named scheduler, broker TTBR0_EL1 at its exit SVC was 0x4000040e91000. The physical root was 0x40e91000, entry page 0x40e96000 and first stack page 0x41296000. The terminal page tables map the executable bytes of four separately hashed ELFs and their stacks to disjoint physical pages. All four process exit SVCs carried their respective ASID/root and exit code. The 24 values in each UART scheduling row match RAM. Columns 12–15 are ready order, 16–19 are cumulative service and 20–23 are remaining slices (zero-based). The verifier checks service changes against elapsed scheduling ticks and owner for all retained rows. Both three-process disabled guests emitted 21 fields per row and independently observed −38 from service 52 in each process.

네 번째 프로세스에서 읽은 TTBR0_EL1, 각 ELF의 실행 코드, 사설 스택을 실제 RAM 페이지표와 대조했다. 4프로세스 실행의 24열과 기존 3프로세스 실행의 21열을 RAM·UART 양쪽에서 확인했다. 기존 프로필에서는 세 프로세스 모두 서비스 52에 −38을 받았다.

The host passed eight broker state tests, including quota, sequence/loss overflow, generations and transaction rollback, plus seven kernel endpoint tests. Seven invalid package selections were rejected before producing a kernel. The original four-slot capture command stopped before QEMU with `expected one to three package slots`. That transcript is retained in `stock-capture.log`. The corrected capture tool was taken exactly from 4f5a8d71c139020c010c5809a5e538728c514ec6. Its two subsequent runs recorded every peer ELF, slot exit and raw RAM. The supplemental C probe's first compile used int64_t, which the bounded SDK does not declare. The corrected probe uses long, with the original compiler diagnostic retained in `extra-pub.first.log`.

호스트에서 구독 한도·계수 오버플로·중복 반영 방지를 포함한 상태 시험 8개, 커널 엔드포인트 시험 7개, 잘못된 패키지 거부 사례 7개를 확인했다. 기존 수집 도구는 4슬롯 매니페스트에서 실행 전 실패했고 원본 로그를 보존했다. 수정 도구로 두 정책을 다시 실행했다. 추가 C 시험의 최초 형식명 오류와 수정 결과도 보존했다.

## Reproduce / 재검증

Extract the archive using tar, then run the following in its top directory. The offline command requires Python 3.9+ and GNU aarch64-linux-gnu-nm. The NM environment variable can point to an equivalent nm. SHA256SUMS includes all capture payloads, package artifacts and scripts.

```sh
sha256sum -c SHA256SUMS
python3 replay.py . > /tmp/pubsub-replayed.json
cmp decoded.json /tmp/pubsub-replayed.json
```

원본 터미널 RAM, ELF, UART, 레지스터 기록과 파일별 SHA-256이 포함된다. 위의 명령은 QEMU 재실행 없이 주소, 바이트, 토큰, 엔드포인트와 열 폭을 다시 판독한다.

Clean build source 51a9ca0e80f1b5510bda6440680983b1eb72c433. Public code 143b1a8c5a997d476e7fed7e36d1b1426ec6f602 differs only in the four documentation and traceability paths listed in `public-diff.txt`. The contract was read at 9a7e4a251e0bc7b2e58628e3ac100528c1483b9f and issue 384. Exact launch commands, package flags, ELF hashes and tool versions accompany each run. These observations cover local software IPC on QEMU's TCG Cortex-A72, one virtual CPU. The instrumented runs establish ordering and process results at the sampled deadlines.

클린 소스와 공개 코드의 차이는 문서·추적 경로 네 곳이다. 패키지와 각 실행의 명령·도구 버전·해시를 보관했다. 측정 대상은 단일 가상 CPU의 로컬 IPC다. 기한 시험 결과는 해당 실행에서 관측한 사건 순서와 반환값이다.

[Raw evidence / raw archive](native-pubsub-ipc-first.tar.gz) · SHA-256 `b39a84dfc47e700c5131b9a266fa7e74e085972e720c9db1f78613a89c104d50`
