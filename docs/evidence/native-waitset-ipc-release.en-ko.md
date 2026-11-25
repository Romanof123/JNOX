# IPC-004 independent release decision · 독립 릴리스 판정

**Pass / 승인.** The frozen #373 waitset contract passes for the AArch64 QEMU virt single-core FIFO/RR process profile on candidate `d33b49ee0d3daac51af70d90b4c4a2e4b5dc573c`, tree `13778a42bb5d9f11072bcef5e9ec2d2336d17e25`. Issue #24 contains further IPC families with separate acceptance decisions. / 명시된 후보의 단일 코어 QEMU virt FIFO·RR 일반 프로세스에서 #373 대기 집합의 검증 기준을 충족했다. #24의 후속 IPC 기능은 별도 인수 대상이다.

The reviewer independently checked out the exact clean source, compiled the original Rust receiver and C sender, packaged each policy with the same ELF pair, and cold-booted both packages. Candidate host tests passed 3/3. The first verifier separately captured 16 runs, including the two original workload boots, six earlier IPC baseline boots and eight supplemental boots. The release decoder also replayed the first verifier’s two original boots. / 독립 검토자는 깨끗한 소스에서 원래 Rust 수신기와 C 송신기를 다시 빌드하고 동일한 ELF 쌍을 FIFO와 RR 패키지에 넣어 각각 콜드 부팅했다. 후보의 호스트 시험 3건이 통과했다. 1차 검증의 16회는 원래 대기 집합 2회, 기존 IPC 6회, 보완 8회다. 독립 판독기로 1차 원본 2회도 재검사했다.

| Fresh release boot / 독립 부팅 | Exits / 종료 | Scheduler rows / 스케줄러 기록 | Timer compare → IRQ / 타이머 비교값 → IRQ | Dropped / 손실 |
| --- | --- | --- | --- | --- |
| FIFO | 71, 72 | 33 | 1,829,963 → 1,840,619 | 0 |
| RR | 71, 72 | 33 | 1,950,991 → 1,963,361 | 0 |

For each release and original FIFO/RR capture, the decoder checked session hashes, walked each process’s own captured page tables and matched all 33 scheduler records on UART against the RAM journal. Each blocked receiver resumed at its saved PC and SP only after the matching set. The first receiver stayed blocked through the mailbox send, pulse send and out-of-mask bit 4. It woke on bit 1, then woke on bit 8 after registering a replacement. Mailbox readiness returned the same handle twice before consumption. The queued pulse survived detach, and the event and mailbox consumers returned the expected original values. The receiver recorded handles 16, 36, 56, 76, 92. Handles 76 and 92 share a slot. Terminal failures were −13 foreign, −116 stale, −11 on both empty polls, and −110 on expiry. The four product words were 0x10000012c, 17, 0x200000200 and 812. Sender status was zero for each of five authorized writes. / 각 캡처의 세션 해시를 확인하고 프로세스별 페이지 테이블로 사용자 결과를 복원했다. UART 기록 33개는 원시 RAM 저널과 모두 일치했다. 우편함·펄스와 마스크 밖 비트에는 대기자가 계속 차단되었으며 일치 비트 1과 8에서만 해당 PC·SP로 복귀했다. 우편함 준비를 두 번 읽은 뒤에야 원래 메시지를 소비했다. 펄스는 등록 해제와 슬롯 재사용 뒤에도 보존됐다. 오류 결과는 소유권 거부, 세대 만료, 빈 조회와 기한 만료를 구별했다.

Source review found fixed-capacity, rotating scans of four slots per process, at most three processes, and one completion per wait. The masked boundary rechecks current endpoint levels at enrollment and after state transitions. Host controls exercise equal-deadline precedence in both orders, capacity, generation exhaustion, stale handles, and exit with a blocked waiter or queued result. Scope is virtual single-core execution plus the host controls. Hardware timing, multicore behavior and same-priority RR quantum rotation were unmeasured. / 소스에서 프로세스당 네 슬롯 순환 검사, 최대 세 프로세스, 대기당 한 결과를 확인했다. 단일 코어의 마스킹 구간에서 등록과 상태 변경 후에 현재 준비 상태를 검사한다. 동일 기한의 경합, 용량·세대 고갈과 차단 중 또는 미수령 결과가 있는 종료는 호스트에서 확인했다. 실기·멀티코어 동작 및 동일 우선순위 RR 퀀텀 회전은 측정 범위 밖이다.

The independently compiled receiver differs from the first receiver in 45 bytes outside its loadable segments. All payload bytes in the executable, read-only and writable ELF load segments match. The C sender has the same whole-file hash. Each kernel is identified by its own digest before boot. / 수신 ELF의 전체 파일에서 달라진 45바이트는 적재 구간 밖에 있다. 모든 적재 구간의 내용은 동일하고 C 송신기는 전체 파일 해시도 동일하다. 각 커널은 부팅 전 개별 해시로 식별했다.

Replay the portable release archive offline with GNU AArch64 binutils, Python and sha256sum. / 아래 명령으로 독립 검증 원본을 오프라인 재판독할 수 있다.

```sh
sha256sum -c SHA256SUMS
python3 replay.py raw
```

The archive contains the two new raw captures, both user ELFs, manifests, build log and checker. The separately sealed first-verifier packet SHA-256 is `cbc9573d90f67ae9f5d3c3db4eeda52008454af037fcd962af303db864457119`. It contains all 16 first-verifier runs and the candidate bundle. / 독립 패킷에는 새 캡처 두 건, 양쪽 사용자 ELF, 매니페스트, 빌드 로그, 판독기가 있고 1차 패킷에는 16회 전체 캡처와 후보 Git 번들이 있다.
