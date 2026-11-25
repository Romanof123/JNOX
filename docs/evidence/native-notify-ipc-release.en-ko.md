# IPC-003 Release decision / 펄스·이벤트 출시 판정

**Release approves IPC-003 for two ordinary AArch64 processes under FIFO and RR on single-core QEMU virt.** Two identical pulse records remained separately readable in FIFO order. Setting the same event bit twice before receipt produced one readable bit. Notification rights remained independent with synchronous calls, mailboxes and notifications active in the same kernel. [CHG-304 / #369](https://github.com/Romanof123/JNOX/issues/369) · [IPC-003 original criteria](https://github.com/Romanof123/JNOX/blob/593a89764ada21b197800f68fe3677f68012e6ca/docs/ipc-mechanisms.en.md)

**단일 코어 QEMU virt에서 일반 AArch64 프로세스 두 개가 FIFO 또는 RR로 동작하는 범위에 IPC-003을 승인한다.** 코드와 값이 모두 같은 펄스 두 개가 순서대로 도착했고, 수신 전 두 번 올린 이벤트 비트는 한 번만 읽혔다. 동기 호출·메일박스·알림을 모두 켠 커널에서도 방향별 송신 권한이 독립적으로 적용됐다. [원래 인수 기준](https://github.com/Romanof123/JNOX/blob/593a89764ada21b197800f68fe3677f68012e6ca/docs/ipc-mechanisms.md) · [네이티브 계약](https://github.com/Romanof123/JNOX/blob/593a89764ada21b197800f68fe3677f68012e6ca/docs/native-notify-ipc.en-ko.md)

## Source and evidence / 검토 소스와 증거

The first verifier and Release executed local commit `10ccba0f3d4aaf9e259eb8713a844e9acc725cb4`, tree `c78ca5be264a3a93c4714830e5787d797ff97e04`. Release independently matched the Git trees for `arch`, `boards`, `kernel`, `sdk`, `tools` and `examples/native-notify` against the public implementation `593a89764ada21b197800f68fe3677f68012e6ca`. The public tree also contains documentation and two synchronous example-case files. Its native notification contract matches the frozen contract in `20f2c338a0ca5e56d3fc59988e7a00c94b1ba7d1`. [Public implementation](https://github.com/Romanof123/JNOX/commit/593a89764ada21b197800f68fe3677f68012e6ca)

Release verified the first archive SHA-256 `7b632cb693ac54cc5fe18fbfc6fb0ecbadd8731c528e7f6da3555abc5a2d37b1`. From a fresh extraction, the checker reproduced all 11 guest outcomes and private RAM values. Separate Release checks bound each UART's two ELF hashes to its packaged binaries and confirmed the kernel hash, frozen manifest, Cortex-A72, one CPU and QEMU TCG. Ten boots used the original fixtures. Nine met their two expected exit values. The original reverse-direction fixture stopped at exit 123 after observing the kernel's EACCES, which its expected errno list omitted. The eleventh boot retained the original source revision and corrected the fixture. All eleven journals reached idle with zero dropped scheduler records.

최초 원본 11개를 새 디렉터리에서 풀어 RAM·UART·패키지 해시를 재검산했다. 처음 10회 중 9회가 두 종료값을 모두 충족했다. 역방향 권한을 확인한 원본 1회는 검사 코드가 EACCES를 예상 목록에서 빠뜨려 일찍 종료했다. 이 원본을 보존했고 검사 코드만 수정한 별도 부팅에서 나머지 조건까지 통과했다. Release가 추가한 게스트 1회까지 합하면 총 12회다.

## Observed behavior / 실행 결과

| Case / 사례 | Execution / 환경 | Result / 관측 |
| --- | --- | --- |
| Supplied C sender and Rust receiver / C·Rust 예제 | FIFO and RR | Both exited 61/62. Pulse codes 17, 17, 23, 23 and full-width words 4294967596, 8589935104, 12884902145, 17179870209 survived sender exit. Channel sums were 812 and 1282. The fifth send and empty poll returned −11. Owner peak was four. |
| Repeated and selective events / 비트 병합·선택 소비 | FIFO and RR | Sets of 9, 1, 4 preceded all takes. Taking masks 1, 1, 12 and all bits yielded 1, −11, 12, −11. Guest UART showed zero initial and terminal latch. Host state inspection recorded the intermediate pending masks 9, 9, 13, 12, 12, 0. |
| Waiting owner / 대기 중인 수신자 | FIFO and RR | The receiver blocked on an empty pulse FIFO. Direct send resumed it with the code and complete u64 word while peak occupancy stayed zero. Its next wait, on bit 31, survived an unrelated bit-3 set. Bit 31 woke it once. The receiver later consumed bit 3. |
| Finite expiry / 유한 기한 만료 | FIFO guest | Pulse and event waits returned −110 after their deadlines. A subsequent late pulse and high bit survived pre-expired receive attempts and arrived at the next polls. Equality and the adjacent one-tick cases for both mechanisms passed in the deterministic kernel state-machine driver. |
| Width and teardown / 인자 폭·종료 정리 | FIFO guests | Invalid arguments returned −22. A denied reverse direction returned −13. After the owner exited with an outstanding record and bit, subsequent sends returned −32 and terminal queue/latch were zero. Forced teardown of a blocked owner and independent selection of all 32 bits passed on the host. |
| Separate rights / 권한 분리 | FIFO guests and Release RR guest | Both call-only and mailbox-only packages returned −38 for the four disabled selectors. All three services also succeeded on one route in the first packet. The added RR case checked both directions with different rights active, including authorized mailbox delivery and synchronous reply. |

The supplemental RR boot granted notifications from slot 0 to 1 and granted synchronous calls and mailbox sends from 1 to 0. Slot 0 filled two queue entries, leaving exactly two free. Two pulses with code `0xffffffff` and word `0xdeadbeef12345678` filled those entries. A fifth send failed. Slot 0 then set bit 31 twice. Its own empty receive returned EAGAIN for both mechanisms while slot 1's data remained pending. Slot 1 attempted a pulse and bit set in the direction authorized for its call and mailbox. Both returned EACCES. Its FIFO contained the two earlier distinct entries and both identical records in order. Its first all-bits take returned only bit 31 and its second returned EAGAIN. The two unauthorized sends from slot 0 using mailbox and synchronous calls each returned EACCES. Slot 1 then sent mailbox word 41 and called with word 42, obtaining reply 43.

Release checked both process-private result arrays, the packaged ELF hashes in the boot UART, four start/end endpoint records, and the 28 scheduler decisions. Slots exited 75 and 76. The terminal owner had zero records, peak four and zero bits. The final scheduler was idle with empty ready queues and zero dropped entries. The Release fixture's first compilation used integer macros absent from the native SDK. Replacing these fixture literals allowed the first Release guest to boot. The original fixture and compiler diagnostics accompany the raw data.

RR 추가 실행에서 0→1에는 알림만, 1→0에는 호출·메일박스만 허가했다. 남은 두 슬롯에 같은 펄스를 두 번 넣고 같은 비트를 두 번 올렸다. 수신자는 기존 두 레코드와 중복 두 레코드를 순서대로 받았고, 비트는 한 번 읽은 뒤 빈 상태를 확인했다. 역방향 알림 두 종류와 정방향 호출·메일박스는 각각 EACCES였다. 허가된 메일박스 값 41과 호출·응답 값 42·43은 정상 전달됐다. 두 프로세스의 전용 메모리와 스케줄러 기록에서 종료값 75/76, 최대 점유 4, 최종 빈 큐·래치를 확인했다. 초기 검사 소스의 정수 매크로를 SDK 지원 리터럴로 바꾼 뒤 처음 부팅했다.

## Bounded resources / 자원 상한

Each owner holds four records with a u32 code and u64 value, one u32 latch and at most one outstanding receive. One send targets one owner. Queuing copies one record into the fixed ring. A waiting pulse receiver receives the record directly. On this two-process profile the deadline and saved-result scans each visit at most two slots per pass. All notification storage is static. In the Release kernel the linked notification-state symbol occupies 296 bytes for both owners and their route state.

Release linked a second kernel with identical application ELFs, compiler, RR parameters and reverse call/mailbox grants and omitted the notification grant for the footprint comparison. GNU text grew from 78,232 to 80,272 bytes, initialized data from 2,408 to 2,704 bytes, and BSS measured 8,699,904 bytes in each image. The two loadable segments contained 2,328 additional file bytes in total. The page-aligned writable segment reserved 8,704,000 bytes in both images. ELF files including metadata measured 181,584 and 181,680 bytes. These measurements apply to the named two-process package.

소유자별 상한은 펄스 네 레코드, 32비트 래치 하나, 미완료 수신 하나다. 한 번의 송신은 소유자 한 명에게 전달된다. 같은 앱·정책·컴파일러로 비교한 결과 알림 상태 전체가 초기화 데이터 296바이트를 사용했고, GNU text는 2,040바이트 증가했다. BSS와 페이지 정렬된 쓰기 가능 영역의 예약 크기는 두 이미지에서 같았다. 시간 동률 및 대기 소유자 강제 종료는 호스트 관측이고, 게스트 검증 범위는 QEMU 6.2 단일 코어 실행이다. 실물 보드 시간 보증·다중 코어·제품 안전 검증은 별도 범위다.

## Reproduction / 재검산

Use the original packet's `python3 verify_results.py .` and the supplement's `python3 check_first.py /path/to/extracted-first-packet` for the first 11 boots. From the extracted supplement run `python3 check_release.py output` with AArch64 GNU binutils installed. Its output reproduces `results.json`. To recreate the additional package and guest use `bash replay.sh /absolute/new/output /absolute/clean/candidate` in a configured JNOX build environment.
