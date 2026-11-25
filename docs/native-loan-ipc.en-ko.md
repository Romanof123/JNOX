# IPC-005 · Native shared-buffer loans / 네이티브 공유 버퍼 대여

CHG-306 specifies a CPU-only profile for ordinary native processes on single-core AArch64 QEMU virt under FIFO or RR. Exactly one `--loan-route PRODUCER:CONSUMER` names two distinct configured slots. An optional third process has no loan rights. Three 4 KiB backing pages transfer payloads without copying them between processes. A two-entry single-producer/single-consumer (SPSC) ring holds committed loans in FIFO order. The kernel owns its indices, descriptors, rights and generations. [Issue #374](https://github.com/Romanof123/JNOX/issues/374) · [Original IPC-005 acceptance #24](https://github.com/Romanof123/JNOX/issues/24)

CHG-306은 단일 코어 AArch64 QEMU virt에서 FIFO·RR로 실행되는 일반 네이티브 프로세스의 CPU 전용 계약이다. `--loan-route PRODUCER:CONSUMER` 한 개로 서로 다른 두 슬롯을 지정한다. 세 번째 프로세스에는 대여 권한이 없다. 4 KiB 페이지 세 개를 사용해 프로세스 간 페이로드 복사 없이 버퍼를 전달한다. 두 칸짜리 단일 생산자·단일 소비자(SPSC) 링은 게시된 대여를 FIFO로 보관하며 커널이 인덱스·기술자·권한·세대를 소유한다.

| Native SVC 0x4a01 | Inputs / 입력 | Result / 결과 |
| --- | --- | --- |
| x8=33 acquire | Producer / 생산자 | x0 status, x1 fresh handle, x2 writable VA / 상태·새 핸들·쓰기 주소 |
| x8=34 publish | x0 writer handle, x1 byte length / 쓰기 핸들·바이트 수 | x0 status / 상태 |
| x8=35 receive | Consumer / 소비자 | x0 status, x1 handle, x2 read-only VA, x3 byte length / 상태·핸들·읽기 전용 주소·바이트 수 |
| x8=36 release | x0 handle for own uncommitted write loan or active read loan / 자신의 미게시 쓰기 또는 활성 읽기 핸들 | x0 status / 상태 |

Each pool slot occupies one page of the user aperture beginning at VA 0x01600000. Keep page tables kernel-private and user mappings non-global and non-executable. Zero a page before granting a fresh writer exclusive RW access. On publish, validate ownership, generation, length (at most 4096 bytes) and ring space before transferring ownership. A full or oversized publish leaves the same writer, contents and queue intact. Revoke the RW mapping and complete the required barriers and translation invalidation (TLBI) before publishing the descriptor with release ordering. Clear bytes beyond the declared length before granting the receiver access. A queued page has no writable user alias. The consumer acquires the published descriptor, then receives its own RO mapping of that page and the original length. Revoke access and synchronize translations before reclamation or reuse. Keep Rust borrows within the ownership interval and confine unsafe page-table operations to the mapping boundary.

풀 슬롯마다 사용자 VA 0x01600000부터 시작하는 영역의 한 페이지를 사용한다. 페이지 테이블은 커널 전용이고 사용자 매핑에는 비전역·실행 금지 속성을 둔다. 새 쓰기 권한을 부여하기 전에 페이지를 0으로 지우고 생산자에게만 RW 매핑을 허용한다. 게시 시 소유자·세대·길이(최대 4096바이트)·링 여유를 먼저 검사한다. 포화나 길이 초과로 실패하면 쓰기 권한·내용·큐를 보존한다. RW 매핑을 철회하고 필요한 배리어와 TLB 무효화(TLBI)를 완료한 뒤 release 순서로 기술자를 게시한다. 소비자가 접근하기 전에 선언 길이 뒤의 바이트를 지운다. 큐에 들어간 페이지에는 쓰기 가능한 사용자 별칭이 없다. 소비자는 acquire 순서로 기술자를 읽고 동일 페이지의 RO 매핑과 원래 길이를 받는다. 회수·재사용 전 접근을 철회하고 주소 변환을 동기화한다. Rust 참조 수명을 소유 기간으로 제한하고 unsafe 페이지 테이블 조작을 매핑 경계에 둔다.

All four operations are nonblocking. No free page, a full ring on publish, or an empty live ring on receive returns EAGAIN. Use wrapping indices with a bounded head–tail distance; publish data before making an entry visible and return ring capacity only after dequeue. A stale, reused or foreign handle cannot change authority or queue state. Producer exit revokes unpublished writers; a surviving reader can drain committed entries, then receives EPIPE on the empty channel. Consumer exit retires the channel and revokes outstanding access before reclaiming each page once.

네 연산 모두 비차단이다. 빈 페이지 없음·게시 시 링 포화·생존 채널의 빈 링 수신은 EAGAIN을 반환한다. 순환 인덱스의 head–tail 거리를 링 용량 안에 유지하며 소비자가 볼 수 있게 하기 전에 데이터를 게시하고 dequeue 후에만 큐 공간을 돌려준다. 과거 세대·재사용·타인 핸들은 권한이나 큐를 바꿀 수 없다. 생산자 종료 시 미게시 쓰기를 철회하되 생존 소비자는 게시분을 비울 수 있고 그 뒤 빈 채널 수신은 EPIPE다. 소비자 종료 시 채널을 종료하고 남은 접근을 철회한 뒤 페이지를 하나씩 한 번만 회수한다.

## Candidate acceptance / 인수 대상

Use ordinary C and Rust apps under FIFO and RR to write and read the actual pages. Verify fresh-page zeros, the published payload and cleared tail, private page tables and permission changes. Fill both ring entries, retain the third writer on failed publish, free one slot, retry and check FIFO order over repeated wraps. Cover producer loss with queued and unpublished loans, consumer loss with an active reader, stale release after reuse and an unauthorized third process. Record private results, actual guest mappings, peak occupancy and final reclamation. Use deterministic host controls for integer-counter wrap. Cross-core visibility on physical hardware and DMA completion/cache maintenance require their respective board/SoC evidence. [Original profile cases](ipc-mechanisms.en.md)

일반 C·Rust 앱으로 FIFO와 RR에서 실제 페이지를 쓰고 읽는다. 신규 페이지의 0 초기값, 게시 데이터와 지워진 꼬리, 전용 페이지 테이블과 권한 변화를 확인한다. 두 칸을 채운 뒤 세 번째 게시 실패가 쓰기 소유권을 보존하는지 확인하고 한 칸을 비워 재게시한 뒤 여러 차례 순환해도 FIFO 순서가 유지되는지 본다. 게시분·미게시분이 있는 생산자 종료, 읽기 중 소비자 종료, 재사용 후 오래된 반환, 무권한 제3자를 구분한다. 사설 결과·게스트 매핑·최대 점유량·최종 회수를 기록한다. 정수 카운터 경계의 순환은 결정론적 호스트 대조로 확인한다. 물리 코어 간 가시성과 DMA 완료·캐시 관리는 각각 해당 보드·SoC의 검증 범위다. [기존 인수 사례](ipc-mechanisms.md)
