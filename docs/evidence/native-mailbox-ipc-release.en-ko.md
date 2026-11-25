# IPC-002 Release decision / 메일박스 승인 판정

**Approved for two native AArch64 applications under FIFO and RR on single-core QEMU virt.** The two applications may use directed mailbox routes alongside the synchronous IPC profile. This decision covers the four-slot, one-u64-per-slot contract in [#367](https://github.com/Romanof123/JNOX/issues/367) and the mailbox family in [#24](https://github.com/Romanof123/JNOX/issues/24).

**단일 코어 QEMU virt의 AArch64 네이티브 앱 2개 구성에서 FIFO와 RR 사용을 승인한다.** 승인 범위는 지정된 방향의 송신, 프로세스당 네 슬롯, 슬롯당 64비트 한 워드 및 동기 IPC와의 공존이다. 실물 보드, SMP, 제품 시간 보증과 기능안전 승인은 별도의 평가 대상이다.

## Frozen inputs / 검토한 소스

The 12 first captures and both Release captures identify local commit `1f9a56354e5b8fa04251fc5101e28ec0d83dad85`, tree `b50b4ce3969715a3229924011f976ef5fee794eb`. Release verified that the kernel, SDK, package and capture tools, mailbox examples and exercised synchronous examples match public source `ced837c6349a62cee081bf537b0d5a347a86995f` and integrated revision `e1d02d08f8bfb37905596bb12a367daef79bcf44`. The captures used the local revision. The full public revisions also contain documentation and additional synchronous example cases.

Release verified the SHA-256 of the first packet, `38d8b31e67ff2e1f2da083fddb0e4a72a542264c71a8ac3e0f8bb25523cf1462`, and its report, `a3e7a5a2bb858380c8af794cdb38e238194e55b7582aa98b242080e4acc4e189`. Running the archived checker against a fresh extraction reproduced its results file byte for byte. The separate failed relative-sleep harness was preserved in that packet. The 12 retained passing boots use the corrected absolute-sleep control where applicable.

## Guest evidence / 게스트 실측

| Case / 사례 | FIFO and RR, PID 0 / PID 1 | Observation / 관측 |
| --- | --- | --- |
| Original strict case / 원래 네 워드만 수신 | 71 / 72 | Four distinct words filled the endpoint. The immediate send returned EAGAIN. A separate finite send timed out and the sender exited before draining. The receiver checked zero, the maximum u64 and two further words in order, followed by EAGAIN. The sender made no later send. |
| Supplied C sender and Rust receiver / C·Rust 통신 | 51 / 52 | Private RAM held send results 0, 0, 0, 0, −11, −110, 0. A subsequent indefinite send was promoted at the first dequeue. The sender exited and the remaining words still arrived in FIFO order, including the promoted word. |
| Independent finite promotion / 유한 기한 내 승격 | 76 / 77 | Release built two C ELFs in a fresh checkout and cold-booted each policy. A full-queue fifth send blocked with a two-second deadline. The first dequeue resumed the sender before that deadline. The receiver read 100, 101, 102, 103, 104 exactly once. A subsequent twenty-millisecond empty receive entered idle, woke at or after its deadline with −110, and an immediate retry returned −11. |
| Owner exit / 소유자 종료 | 73 / 74 | A full endpoint closed with one sender blocked. That sender resumed with EPIPE. A subsequent send returned EPIPE. |
| Independent route directions / 방향별 권한 | 61 / 62 | Both inverse directions returned EACCES. Direct handoff to a waiting receiver copied the full word with peak occupancy zero. The same two tasks completed a synchronous call/reply in the other direction. |
| Disabled mailbox / 미설정 메일박스 | 75 / 75 | Both processes observed ENOSYS for send and receive in a sync-only package. |
| Synchronous regression / 동기 통신 회귀 | 41 / 42 | Both policies returned the expected C/Rust reply and private RAM contained the three expected reply statuses. |

For the two additional guests, Release independently checked both ELF hashes against the boot UART, all capture-session hashes, and process-private arrays via the dumped translation tables. Each UART contains 23 scheduling decisions, zero drops and owner occupancy zero with high-water mark four. The return timestamps below are samples taken by the applications after the kernel resumed their saved frames. Times are absolute monotonic nanoseconds in the guest.

| Policy | Fifth-send deadline | Sender resumed | Empty-receive deadline | Receiver resumed |
| --- | ---: | ---: | ---: | ---: |
| FIFO | 2,003,385,344 | 64,288,016 | 84,467,552 | 84,593,856 |
| RR | 2,003,426,480 | 64,445,888 | 84,672,160 | 84,831,568 |

독립 배포 검토는 기존 12회 원본의 결과를 다시 산출한 뒤, 유한 기한이 있는 송신 승격과 빈 수신의 실제 시간 초과를 위한 게스트를 정책별 1회씩 추가로 부팅했다. 두 정책 모두 다섯 번째 송신이 첫 수신에서 깨어났고 송신자 종료 후에도 수신자가 확정된 워드를 순서대로 받았다. 절대 기한과 복귀 시각, 상태 및 다섯 워드는 각 프로세스의 메모리에서 확인했다.

## Boundary evidence / 경계 조건의 근거

The first packet's three-task host driver checks a release before, at and after a finite deadline. Expiry wins at equality. Two queued senders preserve order, each task can retain one pending word, and a completion can be taken once. Forced exit of a blocked sender cancels its pending word while earlier committed words survive. Closing the owner clears occupancy, wakes remaining senders with EPIPE, and preserves an already due timeout. These are deterministic tests of the kernel mailbox state machine.

The runtime source advances synchronous and mailbox state on the same monotonic time sample, converts absolute deadlines to counter compares with upward rounding, and programs the earliest scheduler or IPC compare. The mixed guest exercised both services with opposite routes. Simultaneously pending finite deadlines in the two different IPC families were examined in source. UART terminal mailbox records expose owner ID, occupancy and high-water mark. The host driver checks pending-state reclamation through completion and blocked-state assertions. Strict four-only and mixed/teardown guest controls check statuses and payloads inside their ELFs and expose pass/fail through PID exit values. The supplied mailbox and synchronous cases and the two added controls also expose private RAM variables.

정확히 기한과 같은 시각의 만료 우선순위, 대기 중인 송신자 강제 종료, 두 대기 송신자의 순서는 동일 커널 상태 전이 코드를 사용한 호스트 검증 근거다. 종료 시 게스트 UART에는 소유자, 현재 점유량, 최대 점유량이 기록된다. 대기자 상태의 회수는 호스트 단언으로 확인했다. 두 IPC 계열에 동시에 유한 기한이 있을 때의 최소 타이머 선택은 소스에서 확인했다. 승인 범위의 실행 환경은 QEMU 6.2, Cortex-A72 가상 CPU 1개, 일반 프로세스 2개다.

## Recheck / 재검산

Unpack the supplemental archive. From its root run `python3 check_release.py output` with GNU `aarch64-linux-gnu-nm` available. To rebuild the two Release guests, use `bash replay.sh /absolute/path/to/new-output /absolute/path/to/clean-candidate`, then run the checker on that output directory. The first packet includes the Git objects required to reconstruct the tested local commit from the public source.
