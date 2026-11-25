# IPC-010 native permit phase timing / 네이티브 허가 위상 실측

**Retain FIFO and RR timing refusal.** At 62.5 MHz every A0 in the paired cohort exceeded 62,500 ticks. Healthy first-phase median upper endpoints fell from 1.978 to 1.361 ms on FIFO and from 1.795 to 1.618 ms on RR. Both builds had 40 definite misses among 512 fully measured controller phases. The figures describe one emulator, these workloads and this sequence of cold boots. WCET, physical timing and operational admission remain unestablished.

**FIFO와 RR 모두 1 ms 인수 거부를 유지한다.** 정상 첫 위상의 중앙값은 두 정책에서 줄었지만, 각 빌드 512개 위상 가운데 40개가 같은 기준을 초과했다. 독립 실행 모두 첫 A 위상에서 이미 기준을 넘겼다. 작업 구성과 입력 도착 시점이 다음 B 위상의 비교에 영향을 준다.

Baseline c5e6a374a9cc8107bfadb25db9c71f8d0d1666e2. Measured candidate d2bd39091eb2d62523dbfa528d864a8ed260ef33. Its [public source](https://github.com/Romanof123/JNOX/commit/f30db111e7b020ec712f268e3e7b0814ab01a926) has matching tracked build inputs. The full tree comparison found four added evidence files under docs/. Every run packaged the same three previously frozen ELFs from c5ab397e; the watchdog and ordinary loan controls each used their own fixed app binaries. Every primary capture bound the three UART boot digests to packaged slot hashes and those ELF bytes.

## Paired stock results / 짝지은 순정 QEMU 결과

Positions 013–044 used old, new, new, old, new, old, old, new within each FIFO/RR × healthy/overrun setting. The 32 launches used serial fresh QEMU processes, plain default-clock TCG, one Cortex-A72, virt/GICv2, 128 MiB, rank 7 controller > 6 sender > 4 supervisor and RR quantum 625,000 ticks. No plugin, synthetic clock or warm-up preceded A0 inside the guest. The 1.1 ms negative workload is the original preemptible A4 computation, with its actual start/end/sampling count in terminal private RAM. All primary settings used watchdog OFF. Unpaired old firsts 001–004 are preserved and excluded from the table.

| Policy and load | Build | A0 upper median | A0 smallest lower–largest upper | B0 upper min–max | B0 misses / 4 | Phase misses / 128 | All-phase max upper | COMPLETE gap max upper |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| FIFO healthy | old | 123,640 | 91,509–132,604 | 31,418–100,631 | 1 | 8 | 132,604 | 210,395 |
| FIFO healthy | new | 85,037 | 82,684–96,816 | 73,018–98,758 | 4 | 8 | 98,758 | 177,729 |
| FIFO overrun | old | 89,022 | 79,043–112,573 | 22,282–86,912 | 3 | 12 | 112,573 | 228,216 |
| FIFO overrun | new | 82,031.5 | 80,388–107,047 | 78,860–98,809 | 4 | 12 | 107,047 | 212,571 |
| RR healthy | old | 112,183.5 | 78,539–135,406 | 28,607–88,543 | 2 | 8 | 135,406 | 225,033 |
| RR healthy | new | 101,104 | 86,169–121,132 | 24,992–128,268 | 3 | 8 | 128,268 | 214,901 |
| RR overrun | old | 124,933.5 | 115,345–131,261 | 19,484–34,940 | 0 | 12 | 131,261 | 230,471 |
| RR overrun | new | 87,427 | 78,339–112,788 | 75,040–101,493 | 4 | 12 | 112,788 | 214,000 |

One tick is 16 ns. Each full phase starts at its nominal release and ends at the scheduler transition that blocks that PID on the next stage-1 absolute wait, or the final receive. All 1,024 primary controller endpoints had prewake blocking, observed ready and actual dispatch witnesses, plus disjoint terminal brackets. Every stored upper finished before the next 156,250-tick phase. Upper bounds for 5 ms jobs were at most 256,881 old / 284,518 new ticks. Sender upper maxima were 266,906 / 295,374 against 625,000 ticks. Max COMPLETE-gap uppers 230,471 / 214,901 met the 312,500-tick check. Max adjacent CHECK operand differences were 168,473 / 174,212. Grid timer compare→return maxima 36,055 / 38,984 exceeded the separate 31,250-tick IRQ allowance in each arm. All primary boots reported 32 successful CHECK/COMPLETE pairs, 8 authenticated publications and receives with matching payloads, 8 accepted protected COMMIT operations, ENABLE=1 at 18/37 ms, and exits 71/72/73. QUAL and SAFETY had zero drops. The FIXED journal dropped 277–371 later rows across both arms. Each retained operation identity uses the protected or qualification journals and only the actual FIXED prefix where needed.

The deliberate A4 full-phase brackets across the four repeats spanned 85,698–102,406 old / 83,643–86,239 new ticks on FIFO and 83,581–87,904 old / 84,042–90,272 new on RR. All 16 overrun spins lasted at least 68,750 ticks in the application’s RAM witnesses. The highest primary all-phase upper bounds were 135,406 old / 128,268 new ticks.

## B0 work and dispatch / 첫 B 위상의 실제 작업

In 32 paired B0s, all 21 successful receives carried a frame, released its handle and produced a successful protected SVC60 COMMIT. All 21 missed. The 11 B0s returning empty EAGAIN all met the phase limit. Six old B0s and fifteen new B0s received a frame. The workload definitions are identical and all boots consumed eight total frames, while their placement across the calendar varied.

| Build and B0 input | Boots | Full B0 inclusive envelope | Target→first dispatch | Dispatch→CHECK | CHECK→COMPLETE | COMPLETE→blocking |
|---|---:|---:|---:|---:|---:|---:|
| Old empty | 10 | 19,439–42,919 | 8,059–27,910 | 1,430–3,840 | 4,001–11,149 | 3,582–6,897 |
| Old frame | 6 | 74,657–100,631 | 3,179–23,099 | 1,333–2,615 | 61,393–73,533 | 4,973–6,926 |
| New empty | 1 | 24,939–24,992 | 13,384–13,501 | 1,908–2,066 | 4,415–4,510 | 5,020–5,127 |
| New frame | 15 | 72,904–128,268 | 1,718–27,033 | 1,199–3,524 | 62,565–89,991 | 4,740–9,822 |

First resumed EL0 execution lies between the final return-assembly counter before ERET and the synchronous user stage-2 mark. The selected owner and PID1 runnable transition are matched by ordinal. CHECK uses the protection decision operand through event 12 journal timestamp. COMPLETE uses event 10's recorded entry/end interval. All intervals include elapsed preemption or emulator pauses. Bracket overlap and aggregation across boots prevent adding table endpoints. The CHECK→COMPLETE span accounts for most of the difference between empty and nonempty B0. Individual rows include per-boot ready, dispatch, receive/release/commit status, handle, sequence and original QUAL IDs in results/b0-by-boot.csv. This sample cannot isolate a causal kernel effect on B0.

새 빌드는 16회 중 15회, 기존 빌드는 6회에 B0에서 프레임을 받았다. 실제 수신 21회에서는 모두 핸들 반환과 보호 COMMIT을 확인했고 모두 1 ms를 넘었다. 빈 큐 11회는 모두 통과했다. CHECK에서 COMPLETE까지 걸린 시간에 가장 큰 차이가 있었다. 각 구간은 선점까지 포함한 경과시간이다.

## Regressions and machine code / 회귀와 생성 코드

Eight extra frozen invalid/live fused boots retained protected operation identity, real private frame rejection and recovery behavior. Each invalid boot received eight inputs, committed three and recorded one private code 2 and four code 17 rejections. Live startup pairs missed exact 625,000-tick spacing in all four boots and remained inhibited at 18/37 ms. Six ordinary loan captures observed eight validated lengths and payload sums, full-ring -11, stale handle -116, dead producer -32, 34 mapping transitions, peak occupancy two and three cleared pages. Thirty watchdog-ON captures ran across old/new FIFO and RR, including six modes on each build/policy. All matched expected role exits and private status, original L+312501 deadlines, masked timer/late-COMPLETE denial where exercised, death inhibition, scheduled generation-2 rearm or live-pair refusal, and virtual write/readback. Both candidate compute-late launches recorded IRQ30 inside the frozen no-call loop. Their timer decisions were D+7,838 FIFO and D+10,130 RR. Candidate sleep-late decisions were D+4,531 and D+3,377. Per-case gate decisions, IRQ PC, fault delays and virtual ENABLE register transitions live in results/watchdog.json and results/boots.json.

For each ref four host loan tests and four host completion-watchdog tests passed, including role/closed/map-failure paths, inclusive timing and clock ambiguity. GNU nm/objdump resolved the three required probe-call sites on all 16 fused package ELFs. For FIFO healthy, protected service caller→entry remained 0x40202c84→{0x40205ad8 old, 0x40205cf0 new}; loan caller→entry 0x40202d24→{0x402062a8,0x40204914}; nested receive caller→entry {0x402063f0→0x402047ac,0x4020496c→0x402064ac}. Static entry allocations in bytes for separate functions were safety::service 128→128, Session::control 208→32, loans::service 224→96, and empty receive 128→0. The candidate has a separate check_at with a 128-byte entry allocation, commit with 144, and queued receive with 112. These are per-function frames; stack depth across calls and translated host time require separate accounting. Old/new first CHECK entry→decision sample envelopes were 18,340–53,587 / 6,430–7,919 ticks in FIFO healthy and 15,944–51,696 / 14,101–45,499 in RR healthy.

## Offline replay / 원자료 재판독

Extract on Linux with Python 3.10+ and GNU AArch64 binutils. After extracting the compact outer archive, restore the ELF and RAM members with `tar -xJf binary-images.tar.xz` in its root. The included `RESTORE.en-ko.md` describes this step. 바깥 압축을 푼 위치에서 내부 ELF·RAM 묶음을 먼저 복원한다. Then run the following commands.

```sh
sha256sum -c SHA256SUMS
PYTHONDONTWRITEBYTECODE=1 bash scripts/replay.sh .
```

Each of the 80 run directories preserves the real UART, RAM, linked kernel, launch argv and executable identities, session hashes, recorded shell command and status. The 44 kernel packages include each build log and manifest; application binaries, detached-source extracts, tracked object listings, tool versions and commands are included. Bootstrap scripts retain host-local paths from this campaign. Replay reads the captured files in their extracted layout. No guest capture was rerun due to failure or excluded from the evidence packet. A first offline provenance command using git show could not traverse an absent parent in this shallow source store; git cat-file and direct tree comparison independently recovered the public commit identity. Raw timings include QEMU translation, virtual work, scheduling and host noise, with no independent board measurement.

파일 해시를 검증한 뒤 재판독 스크립트를 실행하면 80회의 실측에서 위상별 경계·피드백·거부 상태를 다시 얻을 수 있다. 실물 최악 실행시간과 운영 승인은 여전히 미결이다.

[Raw captures and offline replay](native-permit-service-comparison.tar.gz) (SHA-256 `a9c4ed9bc4ffcb1fec160b6cf5ef7cd5c75bd67302f84dee0f4220cf1cf112b7`).
