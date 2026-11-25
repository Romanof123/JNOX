# Independent verification of J-NOX #236 / #232 · 독립 검증

**PASS, scoped to the ordinary virtual workloads. Lane RELEASED.** Public source 952fe28a868c5b7fb0053e76c9772ebfe1122b87. Detached clean worktree. No amendments. Three first-attempt release builds and all six original cold boots retained. Each pair used the identical per-selector ELF. All six guests emitted their terminal records and parked until the declared 15 s timeout (exit 124). Versions and serialized build/launch commands are in versions and run.sh. SHA256SUMS covers the evidence.

| Application | Plain / traced completed chunks at PPI30 | IRQ PC | ELF SHA-256 |
|---|---|---|---|
| Chain | 2102 / 2118 of 16384 | 0x402014d4 | 40fbe24a632215af49e479936b78c31ea16d3d4a2c660dd7fbb75f534aa07569 |
| PI | 42 / 76 of 256 | 0x40202bb0 | fc5be57112e78bb9a4d2de6bb4fc17078331e69c7b26eac10660c9eeea6075ce |
| Ceiling | 80 / 29 of 256 | 0x40202cd8 | 172a88efd01ebda34e6cd05bc32ab4318f814cdec53f40c95d98dc9fb852727c |

BASE held Y and LOW held X while waiting on Y. HIGH's timed X acquire promoted LOW and BASE to 4 ahead of ready MEDIUM. At 62.5 MHz the comparator offset was 62,500 ticks. The IRQ entry samples followed the target by 7,571 and 6,854 ticks. At that point the ready/wait masks, ordered queues and priorities show HIGH alone removed from wait, LOW and BASE restored to 2. HIGH resumed at 0x40201570 with timeout 1 and stored that value; MEDIUM ran, calculated 2,176 and exited; BASE resumed its exact interrupted loop and committed 4,194,304 samples totalling 35,651,584. BASE directly handed Y to LOW; LOW returned successfully, published the same count/sum under X and unlocked Y then X. BASE made 4 MiB of byte reads from the repeated 16-byte input table. The linked loop has LDRB at 0x402014d8, BASE stores at 0x40201520/528 and LOW reads/stores at 0x402012dc/2e0 and 0x402012f8/300.

Independent replay compared ready and wait masks and previously saved frame, PC and x19 for every resumed worker. The supplied replay additionally checks all 11 decisions, full frame ranges, TLS, order, saved x0 and counter sequencing. External trace matched each recorded outgoing exception ELR and all 12 return PCs, plus the terminal BASE exit SVC without a successor. Guest state provides register/result values; the external exception trace provides ELRs and return PCs. Both controls passed the historical decision/data replay (256 cumulative samples of chunk-level progress, lock states, HIGH actual read, MEDIUM sum, ceiling audit). Both controls' IRQ PCs are in their linked sample loops. Host scheduling tests passed 137/137, and five anchor parser tests passed. Replay the extracted evidence with python3 check.py and sha256sum -c SHA256SUMS. Test commands are in run.sh and additional-commands.txt.

**한국어.** 일반 체인·PI·천장 모드의 최초 일반/외부 추적 실행 6회를 모두 보존했다. 체인은 HIGH → LOW → BASE 순으로 우선순위 4를 상속했고, 1 ms 비교기 만료 시 HIGH만 대기열에서 빠지고 두 소유자는 2로 복원됐다. HIGH의 시간초과 복귀, MEDIUM 실행, BASE의 중단 지점 복귀, LOW로 Y 소유권 직접 이전과 보호 합계 게시를 확인했다. 체인 두 실행에서 IRQ 당시 각각 2,102 / 2,118청크를 마쳤다. 비교기·IRQ 시점 차이는 이 가상 실행의 관측값이다. 실기 상한과 WCET는 별도 검증 대상이다. 일반 PI·천장 대조군 네 실행과 호스트 회귀 검사도 통과했다. 검증 레인 해제.

[Original six cold boots and independent replay](ordinary-timed-chain-first.zip) · SHA-256 001d81b80f4e5635676bd6c2f13b31f03d4077e9ff132f226deec726f79353e7.
