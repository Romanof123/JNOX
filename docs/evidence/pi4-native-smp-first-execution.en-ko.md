# Pi 4 two-core first executions / Pi 4 두 코어 최초 실행

Head 60475a0ba7c2b1092305043a65d1f6182625a8ed. Same #322 C/Rust ELF bytes and byte-identical rebuilt calendar. Frozen runner exactly matches the prior cohort after substituting directory names. Pair plain/traced raw hashes match; remote plain/traced raw hashes match. Stock raspi4b QEMU 11.1.3, default TCG/clock/host scheduling, one build/guest at a time. Four documented negative CLI inputs rejected; all four valid builds and six first boots reached terminal. No retries.

| First | A/B productive PPI | Result |
|---|---|---|
| Pair plain | 8/16 | CPU0 A, CPU1 B; complete private results, frames and service. Strict overlap unresolved. |
| Pair traced | 8/16 | Same; external CPU-indexed IRQ headers 18/20. |
| Remote plain | 8/17 | CPU0 targeted SGI1 to CPU1; B interrupted, C clock before cell 2, sleep, positive B work, wake, C exit to B. Strict overlap unresolved. |
| Remote traced | 8/18 | Actual SGI selection and sleep lifecycle, IRQ headers 19/23. Send/receive timestamp ambiguity 421 ticks. Strict overlap unresolved. |
| Swapped plain | 8/16 | A ran on CPU1 and B on CPU0. Strict overlap unresolved. |
| Legacy FIFO | — | Private counts and exits pass; secondary park PCs unsampled. |

Prime counts/exits 78498/12 and 70435/77 on every position. Remote C 32/32. Same user result VA, distinct physical pages/ASIDs. Actual persisted frame comparisons, timer/lease checks, stack guards, stop rendezvous and zero journal loss verified. At each SMP position strict bounded interleaved progress witness was absent, public replay returned 2. Legacy returned 0. No nonzero TLS control or board/host-parallel measurement.

Remote plain bracket [-3164,+771]. CPU0 publish/send 3946661/3946681; CPU1 SGI entry conservative minimum 3949821 after send. Interrupted B candidate 1065733 increased to 1081331 while C slept. C first clock upper bound 3968508 < cell 2 at 5166911. Remote traced bracket [-1633,+1281], send 3962129, receive conservative lower bound 3961708. Time intervals admit ambiguity despite genuine SGI1 receipt replacing B with C. Independently rehashed duplicate grant, release and wrong service were rejected; deleting the remote receive gave an additional chronology gap. Original per-position sessions, kernel dumps, exceptions, disassemblies and exact options in archive.

실제 두 코어의 소수 작업이 각각 실행되고 타이머에 의해 선점됐다. C·Rust 및 달력의 사설 결과와 종료값, 마스크 교환에 따른 실행 코어 변경을 확인했다. 원격 일반 실행은 SGI 인계와 기한 충족을 확인했다. 원격 추적 실행은 송신·수신 시각 불확실성이 421틱 겹쳤다. 다섯 SMP 최초 실행 모두 보수적인 양 코어 계산 겹침 판정은 미확인이다. 기존 FIFO만 판독 종료값 0이었다.
