# IPC-008 integrated release verification / 발행·구독 통합 출시 검증

**Release supports acceptance of local IPC-008 under FIFO and RR on one AArch64 QEMU virt CPU.** Sixteen independently launched, durably captured boots used integrated commit `6fc32ffb1102a4cfa4eb5f4805329b56758aaadf`, tree `c7e73d4a48b0b063cfe46b688955b225dc170ead`. The prospective contract is in `docs/native-pubsub-ipc.en-ko.md`. The original first archive verified all 202 SHA-256 entries and reproduced its complete decoded JSON byte for byte. A separate earlier 14-boot release attempt lost its temporary files after WSL restarted. Its observations are excluded from the sixteen; see `first-position-loss.md`.

단일 Cortex-A72 QEMU virt에서 FIFO와 RR로 실행한 영구 보관본 16건은 IPC-008 승인을 지지한다. 최초 독립 증거의 해시 202개와 오프라인 디코딩을 재확인했다. 별도 14회 실행의 임시 파일은 WSL 재시작으로 유실되어 승인 근거에서 제외했다.

| Workload / 사례 | FIFO and RR private results / 두 정책의 결과 |
| --- | --- |
| C publisher, Rust reader, C stalled reader, Rust broker | Exit 71/72/73/0. Active IDs 1–8 and zero losses. Stalled IDs 1,2,3,4,8, two drops. |
| Deadline expiration / 만료 우선 | Exit 81/82/83/0. Token 26 expired, reply −116, subsequent read ID 1 with A5 once. |
| Ready before deadline / 전달 우선 | Exit 81/82/83/0. Token 26 received ID 1 with A5, reply 0, queue empty. |
| Disabled service, three apps / 서비스 미선택 | Exit 84/84/84. SVC 52 returned −38 in all three address spaces. |
| Simultaneous socket and pubsub selectors / 이중 브로커 | Exit 90/91/92/93. Clients received −13 on both privileged calls. Socket slot 2 received CA02 via SVC 37 and replied DA02. Pubsub slot 3 received CA03 via SVC 52 and replied DA03. Each broker received −13 on the other broker's selector. |
| Native C shell / 셸 | Exit 0/0/0. wc stdout hex 320a. Shell private results [1,1,1,2,1,2,0,0]. |
| Local stream sockets / 로컬 스트림 | Exit 73/74/0. Server read 3/16/45, sum 2080. Client wrote 7 then 57, pressure −11, reverse acknowledgment 32. |
| IPv4 TCP/UDP over isolated TAP / IPv4 | Exit 75/76/0. TCP bytes 9/13 outbound, 12/14 inbound, EOF and −1/104 after peer reset. UDP relay IDs 1,4,3,0 at 10.0.2.2:43004; nine empty polls. Both ordinary Linux peer journals and Ethernet captures retained. |

The eight publications have lengths 0,1,7,32,11,3,19,6. The decoder compares every byte at the publisher and eligible readers, with exact hex in `decoded.json`. Stalled queue observations were [4,4,2,1]. Unsubscribing discarded unread ID 7, retired handle [6,1], and re-enrollment produced [6,2]. Stale/empty were −116/−11. Unauthorized subscribe/publish both returned −13. Both named brokers peaked at two subscriptions, five records, two drops and sequence 8. All queue, subscription and held-wait counts ended at zero. Terminal endpoint state was empty. Named guests had zero peak held waits. The deadline boots establish a held wait with recorded kernel token 26, a later publication token 31 and no intervening reader RPC. Publisher exit woke token 59 or 54 with −32. Foreign observe/unsubscribe both returned −1.

지연 구독자의 최초 네 레코드와 누락 수 2, 해지 후 새 세대에서 8번만 수신한 결과를 사설 RAM에서 확인했다. 발행자·활성 및 지연 구독자의 바이트열을 각각 대조했다. 만료 시험에서는 발행 뒤 만료된 대기에 대한 응답이 거부되어 표본이 보존됐고, 반대 순서에서는 한 번만 소비됐다. 양쪽 모두 발행자 종료 후 보류 대기가 −32로 끝났다.

Both named guests retained 128 × 24 scheduling fields with 1,044 rows dropped per policy. First evidence retained the same width with 1,044/1,045 dropped. Every retained UART row equals its raw RAM row, with slot service charged to the prior owner. Each process used a distinct root, executable backing and stack. The deadline probes retained 128 rows with 158 dropped on expiration and 139 on readiness. Legacy captures retained seven 21-field rows each, zero dropped. Fresh named constant results match first evidence except RR's dropped-row count. Absolute deadlines and the legacy image's physical placement varied. Eight broker-state host tests passed. Seven invalid package selections were rejected.

추적 버퍼 범위를 넘은 행은 복구하지 않았다. 보관된 스케줄링 행은 RAM과 UART가 일치한다. 네 앱의 사설 페이지표와 종료 시 ASID를 대조했다. 셸, 로컬 스트림, IPv4 회귀는 표에 적힌 워크로드를 대상으로 했다.

Pubsub service, client examples and public C/Rust pubsub implementation sources match the first build. The C publisher and stalled subscriber ELFs are identical to first evidence. The broker ELF differs but its .text matches. The active Rust reader .text and integrated kernel differ. Actual sample bytes, queue decisions, deadline token outcomes and scheduler field semantics establish behavioral equivalence. The role probe uses one small C ELF with four process identities to isolate selector routing. The named deployment uses four separately built production/example ELFs. See `equivalence.json` for image hashes and observed differences.

초기 버전과 발행·구독 소스는 같다. C 앱 두 개는 ELF 전체, 브로커는 .text가 일치한다. Rust 활성 구독자와 커널의 바이너리 차이는 원본 해시와 함께 보관했다. 대조 근거는 실제 페이로드, 큐 상태, 토큰·종료·스케줄링 관측이다.

## Offline replay / 오프라인 재검증

Extract the companion archive. Set SRC to the frozen integrated checkout. Install Python 3, pyelftools and GNU AArch64 binutils. From the extracted directory:

~~~sh
sha256sum -c SHA256SUMS
python3 replay.py . > /tmp/release-pubsub.json && cmp decoded.json /tmp/release-pubsub.json
python3 regress.py . "$SRC" > /tmp/release-regress.json && cmp regress.json /tmp/release-regress.json
for p in fifo rr; do
  python3 "$SRC/examples/native-inet-sockets/private_results.py" inet-$p tap-inet-$p tcp.elf udp.elf > /tmp/release-inet-$p.json && cmp inet-$p.json /tmp/release-inet-$p.json || exit 1
done
~~~

The capture logs, register traces, package flags, manifests, private RAM, tool versions, source comparison and actual peer traffic accompany the [raw archive](native-pubsub-ipc-release.tar.gz).

[Raw captures and offline replay](native-pubsub-ipc-release.tar.gz) (SHA-256 `3cb234ed5ac71111f81d0babac6e352ceadc525a5d76cb3db39aa7f9f15c8db0`).
