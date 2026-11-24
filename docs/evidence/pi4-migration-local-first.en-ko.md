# #278 / #279 independent stock-model firsts · 독립 최초 검증

Candidate `ce8aebc872612cb036c05928fc8764d947d76145`. Plan `330dcc228a3bd964c14408bbc438600e8905bb95`. Rust 1.95.0 (59807616e), four-core raspi4b TCG, two active CPUs, stock executable SHA-256 `3ae2c079d4491362fece2abb641f0ff17d88e9008c9fcd13600f249b9bd1c862`. Six images and two positions each fixed before launch. All original sessions are preserved in order. No early terminals.

| Workload and policy | First | Route | IRQ and actual ERET PC | Linked loop [start,end) | In loop | CPU0/1 rows |
|---|---:|---|---|---|---|---|
| short-global | 1 | 0→1 | 0x801e4 | 0x82304–0x82328 | no | 34/34 |
| short-global | 2 | 0→1 | 0x82304 | 0x82304–0x82328 | yes | 34/34 |
| short-partitioned | 1 | 0→0 | 0x82308 | 0x82308–0x8232c | yes | 45/23 |
| short-partitioned | 2 | 0→0 | 0x82368 | 0x82308–0x8232c | no | 45/23 |
| short-clustered | 1 | 0→0 | 0x82088 | 0x823b4–0x823d8 | no | 45/23 |
| short-clustered | 2 | 0→0 | 0x823b4 | 0x823b4–0x823d8 | yes | 45/23 |
| stream-global | 1 | 0→1 | 0x80cec | 0x80cec–0x80d2c | yes | 34/34 |
| stream-global | 2 | 0→1 | 0x80cec | 0x80cec–0x80d2c | yes | 34/34 |
| stream-partitioned | 1 | 0→0 | 0x80cec | 0x80cec–0x80d2c | yes | 45/23 |
| stream-partitioned | 2 | 0→0 | 0x80cec | 0x80cec–0x80d2c | yes | 45/23 |
| stream-clustered | 1 | 0→0 | 0x80cec | 0x80cec–0x80d2c | yes | 45/23 |
| stream-clustered | 2 | 0→0 | 0x80cec | 0x80cec–0x80d2c | yes | 45/23 |

**Strict continuation.** Short qualified 3/6, one crossing and two same-core. Stream qualified 6/6, two crossings and four same-core. All 12 had an observed CPU0 M IRQ and matching ERET on the selected CPU, the same frame/stack/X21, and resumed live M TLS. Three short PCs fell outside the linked arithmetic loop. No first was replaced.

**Core and task state.** For each original, independent.py reads raw rows and the vCPU-thread exception logs. Five saved and five live witnesses bound the actual lease, two distinct CPU bases, four activation pointers/tokens and live prefix values. Independent recomputation matched four terminal summaries in every run. Short M completed 65,536 additions, sum 2,147,516,416, digest 18080812676020141577. Stream M completed 4,194,304 bytes, CRC 0x78d22ad1, digest 8765611335505774534. Original public replay adds complete lease, scheduling, sparse-row, loss and observer checks. See frozen manifests for exact linked addresses, image and tool hashes. A finite checkpoint digest cannot establish every intermediate instruction.

**Linked paths.** Both BSP entries call init, which checks MPIDR, EL and counter, installs the single per-image EL1 base writer and then sets VBAR. cpu_record validates affinity before selecting CPU0/1. local_lease compares observed EL1 against that address. Dispatch uses release store, retirement exchanges the lease, and IRQ/sparse sampling use its checked value. Frame save writes zero into the old EL1 slot and captures SP_EL0 plus both task registers. The common selected/initial restore writes SP_EL0 and both TLS registers before ERET. Six raw opcode and call-site extracts are alongside the linked disassemblies. checked_tls guards pointer and immutable token on each digest access. The checkpoint's rotate/XOR/add updates TLS, and the SVC result loads that TLS. Idle has zero TLS, while both P1 activations begin independently.

**Controls.** Eight public preflight tests passed. Ten mutations of private copies of actual MG3 stream-global rows rejected swapped CPU base, zero/foreign TLS, reused P1 TLS, wrong token, stale claimed/live leases, wrong digest, absent destination live witness and saved-only crossing. Historical public MG1, MG2 and CRC archives each replayed six positions, qualifying 1/6, 0/6 and 6/6. Each disposition and session hash is retained. The generic virt two-core SMP guest reached SMP GLOBAL MIGRATED and SMP PASS after its EL1 task-cookie check. The Pi directed-wake image produced SW END, and the separate bounded SW1 public replay matched CPU-thread PCs and reported no software gaps. The first SW1 smoke used a fixed three-second termination and is separately retained. Wrong board and standalone stream selectors failed. All-features virt completed COOP PASS.

**Scope.** Stock QEMU model only. Physical board, shared-cache interference, target latency limits and acceptance remain open under #47. No public write. Public candidate source for the build and tools is included as tracked files, once. The commit identifies the complete upstream tree and historical input archives.

**한국어 판정.** 고정된 여섯 이미지에서 각 두 번씩 최초 부팅을 보존했다. 짧은 산술 작업의 엄격한 루프 내부 복귀는 3/6개(코어 간 1개, 동일 코어 2개), CRC 스트림은 6/6개(코어 간 2개, 동일 코어 4개)였다. 짧은 작업의 나머지 세 건도 실제 IRQ와 ERET의 PC가 같고 목적지에서 M의 TLS를 다시 읽었지만 중단 PC가 산술 루프 밖이었다. 조기 종료는 없었다.

각 실행에서 저장·실행 증거 각각 다섯 건과 네 활성화의 종료 다이제스트를 확인했다. 두 CPU의 기준 주소, 네 활성화의 TLS 주소와 토큰, 소유 리스, 접두 누적값을 원본 UART와 CPU 스레드 로그로 대조했다. CRC는 4 MiB 전부 처리 후 0x78d22ad1이었다. 과거 형식 재생, 실제 행 사본 변조 10종 거부, 일반 SMP 태스크 쿠키, Pi SW1 직접 모델과 모든 기능 조합의 협력 스케줄러를 확인했다. 실기기와 상위 #47의 승인은 미결이다.

SW1 bounded capture elapsed 1223914108 ns, terminal quiet 1012659183 ns.

[Twelve original migration positions, independent reconstruction and controls / 최초 실행 12건과 복귀·TLS 독립 대조](pi4-migration-local-first.zip) · SHA-256 9593c8e207d995c848cacb646852aba1241b19b2159e8f8d32575c004287b580.
