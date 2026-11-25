# IPC-003 independent first results · IPC-003 독립 첫 검증

Native C and Rust processes returned **61/62 on FIFO and RR** on single-core AArch64 QEMU virt. Each sender filled four pulse slots and received EAGAIN on its fifth send. After sender exit each receiver recovered all four original code/word pairs in order. The recipient consumed a repeated event bit once and subsequently recovered the unrequested bits. Execution revision `10ccba0f3d4aaf9e259eb8713a844e9acc725cb4`, tree `c78ca5be264a3a93c4714830e5787d797ff97e04`. Criteria were frozen from issue #369 and the native notification contract at `20f2c338a0ca5e56d3fc59988e7a00c94b1ba7d1`.

단일 코어 AArch64 QEMU virt에서 일반 C 송신자와 Rust 수신자가 FIFO·RR 양쪽에서 61/62로 종료했다. 다섯 번째 펄스의 거부 뒤에도 원래 네 레코드를 송신자 종료 후에 도착순으로 읽었다. 여러 번 설정된 비트는 한 번만 소비했고, 선택하지 않은 비트도 이후 읽을 수 있었다.

| C sender and Rust receiver private memory · 프로세스 전용 메모리 | Actual result, identical under FIFO and RR · 두 정책 실측 |
| --- | --- |
| Sender statuses | 0, 0, 0, 0, −11, 0, 0, 0 |
| Pulse codes | 17, 17, 23, 23 |
| Full 64-bit words | 4294967596, 8589935104, 12884902145, 17179870209 |
| Channel sums | 812, 1282 |
| Empty pulse | −11 |
| Event results for masks 1, 1, 12, 0xffffffff | 1, −11, 12, −11 |

Process-specific translation tables and ELF symbols locate these values in the original guest RAM. The kernel's boot and terminal notification journals record zero initial occupancy and latch, peak FIFO occupancy four, and zero terminal occupancy and latch. The guest journals show the sender exiting before the receiver's first poll. Intermediate queue lengths and latch states were sampled directly on the host model. The latch advanced 0, 9, 9, 13, then 12, 12, 0 after the first three takes.

전용 페이지 테이블과 ELF 심볼을 이용해 게스트 RAM에서 두 앱의 값을 읽었다. 게스트 커널 로그에서 시작과 끝의 큐·래치는 0, 최대 점유량은 4였다. 중간 래치 값은 호스트 모델에서 직접 확인했다.

| Cold guest case · 게스트 사례 | Slot 0/1 exits | Other observations · 관측 |
| --- | --- | --- |
| Official FIFO and RR, two boots | 61/62 each | Four ordered words, overflow, coalescing, selective take, sender-exit survival |
| Receiver first, FIFO and RR, two boots | 63/64 each | The high-priority receiver blocks on a pulse. Sending the pulse switches back to it. It then blocks on bit 31. Setting bit 3 keeps the sender running. Setting bit 31 switches to the receiver, which later recovers bit 3. Pulse peak zero |
| Finite timeout, FIFO | 65/66 | Both timed waits returned −110 after their deadlines. Later sends left the pulse and bit 31 available. Pre-expired reads returned −110 even with data waiting, and both subsequent polls recovered that data |
| Width, duplicate records and reverse direction, original FIFO | 67/123 | Five invalid send operands returned −22. The verifier exited 123 on the first reverse send because its errno list omitted the actual denial −13 |
| Width, duplicate records and reverse direction, corrected fixture | 67/68 | Both reverse sends returned −13. The receiver rejected zero and over-width take masks (−22), recovered bit 31 and two identical full-width pulse records, and saw −11 after draining |
| Call-only and mailbox-only, two FIFO boots | 69/70 each | In each package both notification sends and both receives returned −38 |
| Call, mailbox and notification grants together, FIFO | 71/72 | Private RAM contains distinct pulse, mailbox, synchronous request and reply words plus bit 31 |
| Owner exit with queued pulse and latched bit, FIFO | 73/74 | The owner exited without a receive. Both subsequent sends to it returned −32. Terminal peak one, zero occupancy and latch |

The ten original cold packets remain intact. Nine met both frozen exit criteria. The original width/reverse control stopped on EACCES, which meets the specified denial semantics. The eleventh packet changes only the test fixture's accepted denial codes and records the remaining controls. All eleven scheduler journals end idle with empty ready queues and zero dropped rows. In the finite control the receiver observed timeout at 13,433,392 ns for a 13,070,992 ns deadline and at 23,671,504 ns for a 23,483,760 ns deadline.

처음 수집한 게스트 패킷 10개를 모두 보존했다. 그중 9개가 양쪽 종료값 기준을 충족했다. 최초 역방향 권한 검사는 실제 거부값 EACCES를 검사 코드가 누락해 중단됐다. 열한 번째 패킷에서 해당 검사만 수정하여 두 역방향 거부와 나머지 경계 사례를 확인했다. 모든 게스트의 최종 스케줄러는 유휴 상태였고 준비 큐와 누락된 로그 행은 0이었다.

Three repository host tests passed. Independent host controls also exercised all 32 individual bits, repeated sets, a second receive while blocked, and a blocked owner's teardown with both queued data and an unrelated latched bit. For pulse and event delivery one tick before deadline, the host receiver completed once. At equality and one tick after, it timed out once and a later poll delivered the original notification. The exact-time race and blocked-owner teardown were observed on the host model. The guest cleanup case observed an exiting owner with pending data. Runtime slot reactivation, physical boards, and product safety qualification were outside this evidence.

저장소의 호스트 테스트 3개와 별도 호스트 검사에서 32개 비트 각각, 이미 대기 중인 프로세스의 재수신, 대기 중인 소유자 종료, 만료와 전달 시각이 정확히 같은 경우를 확인했다. 게스트에서는 대기 시각 전후와 데이터가 남은 소유자 종료를 확인했다. 실물 보드, 슬롯 재활성화, 제품 안전 인증은 이 결과의 범위 밖이다.

The archive includes all original UART/RAM/session files, packaged ELFs and manifests, freeze, commands, receipts, independent probes and replay code. Run `python3 verify_results.py .` from the extracted directory with `aarch64-linux-gnu-nm` available. The candidate Git bundle uses prerequisite `04d699c8a445fc837a423c96ba35825810550c4b`.
