# IPC-004 · Native waitsets / 네이티브 대기 집합

CHG-305 specifies the optional `--waitset` package switch for ordinary AArch64 QEMU virt FIFO/RR processes with a mailbox or notifications enabled. Each process may register up to four sources and obtain one readiness result per wait. A registration observes only the issuing process's own enabled receive endpoint. Underlying IPC grants, inactive syscall results, memory protection and timer behavior retain their respective contracts. Independent Release accepted this single-core virtual FIFO/RR profile under REQ-024, DES-013 and VER-024. [Release decision](evidence/native-waitset-ipc-release.en-ko.md) [Issue #373](https://github.com/Romanof123/JNOX/issues/373) · [IPC acceptance #24](https://github.com/Romanof123/JNOX/issues/24)

CHG-305는 우편함 또는 알림을 사용하는 AArch64 QEMU virt FIFO·RR 일반 프로세스에 선택형 패키지 옵션 `--waitset`을 정의한다. 프로세스당 최대 네 소스를 등록하고 한 번에 준비 결과 하나를 얻는다. 등록은 현재 프로세스 소유의 활성 수신 엔드포인트에 한정한다. 기존 IPC 권한·미활성 호출의 반환값·메모리 보호·타이머 규약을 유지한다. 독립 Release는 이 단일 코어 가상 FIFO·RR 프로파일을 승인했다. [출시 판정](evidence/native-waitset-ipc-release.en-ko.md)

| Native SVC 0x4a01 | Inputs / 입력 | Result / 결과 |
| --- | --- | --- |
| x8=30 attach | x0 owner slot, x1 source, x2 mask / 소유자 슬롯·소스·마스크 | x0 status, x1 opaque generation handle / 상태·세대 핸들 |
| x8=31 detach | x0 handle / 핸들 | x0 status / 상태 |
| x8=32 wait | x0 absolute monotonic deadline / 단조 시계 절대 기한 | x0 status, x1 current handle, x2 readiness / 상태·현재 핸들·준비값 |

Sources 1 and 2 mean mailbox-readable and pulse-readable. Both require mask zero and return readiness 1. Source 3 requires a nonzero u32 event mask and returns the matching bits. Poll with deadline zero or wait indefinitely with UINT64_MAX. Use the established busy, timeout, rights and invalid-input conventions. Foreign registrations disclose no readiness and cannot be detached by another process.

소스 1은 우편함, 2는 펄스의 읽기 가능 상태다. 두 소스는 마스크 0을 요구하고 준비값 1을 반환한다. 소스 3은 0이 아닌 u32 이벤트 마스크를 요구하고 일치하는 비트를 반환한다. 기한 0은 즉시 조회, UINT64_MAX는 무기한 대기다. 기존 사용 중·만료·권한·인자 오류 규약을 따른다. 타인 소유 소스의 준비 상태를 노출하거나 타인의 등록을 해제할 수 없다.

Check current level readiness on enrollment and after endpoint transitions before waking a waiter. Scan at most four registrations, rotating the first choice among ready sources. A wait leaves the message, pulse or event bits for the consuming API. At finite deadline equality, expiry wins. Scope handles to process and generation. Detach and exit invalidate the old generation. Reusing a slot reads the new source's current level; an old ready source cannot supply readiness to its replacement. Fail closed on handle exhaustion and clear pending waits and results on teardown.

대기 등록 시와 엔드포인트 전이 후 깨우기 전에 현재 준비 상태를 재확인한다. 최대 네 등록만 검사하고 준비된 소스 사이에서 첫 선택을 순환한다. 대기 조회 후에도 메시지·펄스·이벤트 비트는 소비 API를 위해 남는다. 유한 기한과 준비 시각이 같으면 만료를 우선한다. 핸들은 프로세스와 세대에 귀속하며 해제·종료 시 무효화한다. 슬롯 재사용 시 새 소스의 현재 상태를 읽고 이전 소스의 준비 결과는 이월하지 않는다. 핸들 고갈 시 등록을 거부하고 종료 시 대기 및 미전달 결과를 정리한다.

## First guest cases / 최초 게스트 사례

Use two ordinary C/Rust apps under FIFO and RR. Queue a message before attach; read readiness twice, consume the original word and poll again. Attach before a later send and observe the blocked waiter resume. Leave a pulse queued, detach its registration and reuse that slot for a masked event. Check that the old handle and out-of-mask bits cannot report the replacement ready. Consume the actual pulse or matching bits and recheck the corresponding source. Exercise foreign-owner refusals and timeout separately. Use host controls for equal-time races, capacity and generation cleanup, and retain their scope alongside the original guest evidence. [Original acceptance cases](ipc-mechanisms.en.md)

일반 C·Rust 앱 두 개를 FIFO와 RR에서 실행한다. 등록 전에 메시지를 큐에 넣고 준비값을 두 번 조회한 뒤 원래 워드를 소비하고 다시 조회한다. 먼저 등록해 차단된 대기자가 이후 실제 송신으로 재개되는 경우도 확인한다. 펄스를 큐에 둔 채 등록을 해제하고 같은 슬롯을 마스크 이벤트에 재사용한다. 이전 핸들과 마스크 밖 비트가 새 등록의 준비 결과를 만들지 않는지 확인한다. 실제 펄스·일치 비트를 소비한 뒤 각 소스를 다시 조회한다. 타인 소유권 거부와 기한 만료도 구분한다. 같은 시각 경쟁·용량·세대 정리는 호스트 대조로 확인하고 게스트 원본과 구별해 기록한다. [기존 인수 사례](ipc-mechanisms.md)
