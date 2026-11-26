# Native physical timer programming trial / 물리 타이머 설정 실험 (#389)

**Keep the 1 ms permit timing gate on hold for FIFO and RR.** Both policies and both builds missed 8 of 128 healthy controller phase deadlines. Every healthy boot missed on the first empty receive, A0, and again on the first accepted frame. The trial eliminated redundant timer writes on observed repeated requests. The selected machine-code path executed more guest instructions in both repeat cases.

**FIFO·RR 모두 1 ms 인수 기준을 충족하지 못했다.** 두 빌드 모두 정책별 정상 콜드 부팅 4회에서 총 128개 위상 중 8개가 마감 시간을 넘었다. 동일한 타이머 요청을 반복할 때 쓰기 명령은 사라졌지만 실행한 게스트 명령 수는 늘었다. 효과는 독립 부팅에서 관찰한 QEMU 결과에 한정된다.

## Default-clock cold boots / 기본 시계 콜드 부팅

One tick is 16 ns. Each median covers four fresh healthy boots. The ready column is the upper bound from nominal release to becoming runnable. Return is the recorded EL1 assembly timestamp immediately preceding return to EL0 on dispatch. A0 and first frame use the upper endpoint through the eventual blocking transition. First frame is selected by sequence and received bytes.

| Policy, arm | A0 release→ready | A0 release→return | A0 release→empty receive journal | A0 upper | First accepted phase upper | First frame at B0 | Misses |
|---|---:|---:|---:|---:|---:|---:|---:|
| FIFO baseline | 17,714.5 | 26,136.5 | 82,580.5 | 102,416.5 | 90,635.5 | 2/4 | 8/128 |
| FIFO trial | 18,554 | 27,652 | 86,950 | 101,261 | 83,254 | 4/4 | 8/128 |
| RR baseline | 19,194 | 30,165.5 | 91,642.5 | 112,432.5 | 88,602.5 | 2/4 | 8/128 |
| RR trial | 17,576.5 | 26,056.5 | 83,173.5 | 99,720.5 | 86,885.5 | 4/4 | 8/128 |

The paired order blocks gave opposite A0 upper-endpoint changes. Subtracting the two baseline observations' mean from the two trial observations' mean within each block gave +12,087.5 then −13,601 ticks on FIFO, and −25,016 then +17,355 on RR. The median first-frame receive→release interval was 15,062→11,985.5 ticks FIFO and 15,769→12,408 RR. The other baseline first frames arrived at A1. The generator, receiver, CRC, copy instructions and supervisor binaries were identical in both arms. Frame placement and the differing block directions limit attribution to timer programming.

All 512 healthy controller phases had a proven pre-release block, ready transition, dispatch return and closed endpoint. Four further overrun boots and four invalid-frame boots brought the fused total to 768 controller endpoints. Each fused boot checked all 32 protected phases, 24 empty receives, eight frame-byte joins and all private decisions. The healthy maximum upper controller job was 259,129 ticks, maximum upper sender endpoint 267,324, maximum protected CHECK gap 173,747, maximum upper COMPLETE gap 227,535 and maximum grid IRQ latency 31,488. Every healthy sender endpoint closed. Qualification and safety loss was zero. The separate bounded fixed-debug ring dropped 277–278 rows on healthy fused runs and 168–721 across all instrumented stock safety cases.

One baseline RR healthy boot, 007, still had output OFF at 18 ms. Its first ON began 1,412 ticks after that observation and it was ON at 37 ms. The other 15 healthy boots were ON at both sample times. Raw output transitions and both observation instants are retained. The healthy sender paired issue timestamps were exactly 625,000 ticks apart.

정상 위상 512개 모두에서 기한 전 대기, 준비 상태 전이, 사용자 모드 복귀 직전 표본, 다음 차단까지의 상·하한을 확인했다. FIFO와 RR 모두 두 순서 블록에서 A0 전체 시간의 증감 방향이 서로 바뀌었다. 기준 빌드의 첫 프레임은 B0와 A1에 나뉘었고 실험 빌드의 첫 프레임은 모두 B0였다. RR 기준 실행 007은 18 ms 표본에서 출력이 꺼져 있었고 1,412틱 뒤 켜졌으며 37 ms에는 켜져 있었다.

## Timer readback and instruction counts / 레지스터와 명령 수

Across 44 stock qualification boots, the changed-request rows read back 744 earlier targets, 1,488 later targets, 80 disables, 80 re-arms and 44 initial requests. Every enabled target equalled the hardware CNTP_CVAL_EL0 readback. CNTP_CTL_EL0 writable bits equalled 1 for arm and 0 for disable. All 1,354 physical timer IRQs had IAR 30, the most recently requested active compare, CTL low bits 5 including ISTATUS, and ordered timestamps from due time through exception return. In trial boots 002 and 005 the B0 grid point was crossed during a sender syscall. The scheduler made the controller ready and dispatched it through reason 34 and 62 respectively, with no distinct timer IRQ for that grid point. The replay preserves these transitions.

Four additional healthy boots used a one-vCPU QEMU execution callback restricted to linked kernel PCs. A contiguous instruction window started immediately after the call to `ipc::next_compare` and ended immediately before the call to `record`. Every observed adjacent PC was checked against decoded AArch64 control flow, including direct calls and returns. The count includes deadline selection after that first call, the timer guard and barriers, qualification comparison and argument preparation for the fixed journal. Each policy reproduced the same counts.

| Request | Calls per diagnostic boot | Executed instructions, baseline→trial | CVAL writes | CTL writes |
|---|---:|---:|---:|---:|
| Repeated same armed target | 316 | 40→48 | 1→0 | 1→0 |
| Repeated disable | 35 | 34→39 | 0→0 | 1→0 |
| Changed armed target, previously disabled | 2 | 62→71 | 1→1 | 1→1 |
| Changed armed target, previously armed | 50 | 66→78 | 1→1 | 1→1 |
| Changed to disabled | 2 | 63→69 | 0→0 | 1→1 |

Both baseline healthy ELFs inlined physical timer writes in `fixed_event`. Their counted window was 0x4020162c through 0x402016b8 exclusive. Both trial ELFs had the counted window 0x40201678 through 0x402016e0 exclusive. The 76-byte linked helper started at 0x402014c8. Its CTL read was at 0x402014d0, writable-bit mask at 0x402014d4, CVAL read at 0x402014e8, changed-target CVAL/CTL writes at 0x402014f8/0x402014fc, disable write at 0x40201508 and final ISB at 0x4020150c. QEMU reported instruction execution on these observed, nonfaulting paths. This measures guest dynamic instructions on TCG. A hardware retirement counter, elapsed instruction costs, and physical-board WCET remain unmeasured.

For positive and negative controls, a separate EL1 exerciser loaded each original trial kernel ELF at its linked addresses, masked interrupts, wrote physical timer preconditions, and called that exact helper. Fourteen vectors per policy covered all four combinations of ENABLE and IMASK, equal/earlier/later CVAL, explicit disable and re-arm, and an already expired compare. Equal armed CVAL skipped both writes with CTL 1 and with CTL 5. A masked active timer and equal CVAL forced both writes. An earlier or later target forced both writes, and a disable with any writable bit set forced the CTL write. All 28 vectors passed independent before/after CVAL/CTL readback and instruction-path checks. These two exerciser launches isolate register behavior. The four observer launches and their timing are excluded from the primary comparison.

실행 경로 추적에서 정책별로 동일한 활성 요청 316회, 동일한 비활성 요청 35회를 확인했다. 각 경우 게스트 실행 명령은 40→48개, 34→39개였고 타이머 레지스터 쓰기는 2→0회, 1→0회였다. 별도 EL1 시험은 실제 링크된 함수에 대해 정책별 14개 조건에서 ENABLE, IMASK, 만료 상태, 이전·이후 기한과 비활성화 동작을 확인했다. 정상 부팅에서는 2회에 한해 B0 도달 시 진행 중인 시스템 호출이 깨우기와 디스패치를 수행했다.

## Fault and ordinary controls / 고장·일반 경로

Each of the four real A4 overrun injections spent 68,814–68,991 ticks in preemptible guest work and missed 1 ms on A4. Invalid frames produced five private rejections after sequences 1–3, without admitting sequence 4. All 20 watchdog boots passed. The compute-late IRQ interrupted the EL0 compute loop and sleep-late interrupted the EL1 idle continuation (saved exception level EL1h, idle flag 1). Each due event was joined to its enclosing IRQ 30 and output-OFF write. A late COMPLETE returned −110. The four deterministic recovery boots saw the exact 625,000-tick pair and ON/OFF/ON/OFF. All four actual-live recovery attempts reacquired generation 2 and refused re-enable (−11), with measured pair separations 635,317 and 624,381 ticks on FIFO baseline/trial, and 621,016 and 625,557 on RR baseline/trial. They retained OFF. Successful live recovery with an exactly spaced naturally observed pair was not demonstrated.

All four ordinary-loan boots produced eight successful transfers, eight expected payload sums, 34 matching page-map transitions, full queue −11, stale handle −116 and final peer-closed −32, with every slot unmapped at termination. The ordinary ELF bytes differed between kernel builds. Behavioral checks cover the observed loan contract.

4회 실제 과부하 시험에서는 선점 가능한 A4 작업이 모두 1 ms를 넘었다. 감시 타이머는 실행 중인 사용자 연산과 커널 유휴 대기 양쪽에서 IRQ 30을 발생시키고 출력을 껐다. 정확한 시각쌍을 넣은 복구는 네 번 모두 출력을 다시 켰다. 실제로 읽은 시각쌍은 네 번 모두 625,000틱과 달라 세대 2에서 재활성화가 거절됐고 출력은 꺼진 상태였다. 일반 대여 IPC 네 번은 전송 8건, 매핑 전이 34건, 만료 핸들과 종료 상태를 모두 확인했다.

## Reproduction / 재판독

Measured baseline a7aa0a659f239950fa29b3326813cf4b35d846f9 and measured trial 5d4f6548874548ffb773e3935e35c2576c03007c differ in one file, `kernel/src/native_el0.rs`. Public trial 537480b54357c0c742429540ee3f604fe29f6533 has the same full tree 2a16757fe9961f368af1e23527cf485a341827df. Both frozen commit objects and tracked build-input trees, a public commit API receipt, the source diff, all 36 package manifests, build logs and linked binaries are included. The frozen optimized controller SHA-256 is 27a6549f50044f80778322bbfa0922e19e1dedbcdf6847749895c3e397512e78. All six input hashes are listed in `scripts/fixtures.sha256`.

QEMU 6.2.0, virt/GICv2, a single Cortex-A72 under TCG, 128 MiB, default clock, no warm-up or plugin on the 48 primary launches. Priorities were controller 7, sender 6, supervisor 4, with RR quantum 625,000 ticks. Positions 001–016 used FIFO ABBA / RR BAAB followed by FIFO BAAB / RR ABBA. A is baseline. The other 32 boots were controls. No primary launch was discarded or repeated. The QEMU plugin CLI appended `=on` to positional arguments, so the original recorded PC files carry that suffix. Source assembly and disassembly, actual launch arguments, observer records and raw UART/RAM are retained.

From the extracted directory, run `bash scripts/replay.sh .` using Python 3, AArch64 GNU nm/objdump and sha256sum. It verifies all retained SHA-256 digests, reconstructs timing rows, code paths, fault and loan results, and rechecks the digests. The campaign and diagnostic launch scripts retain the local fixture and build paths used for acquisition.

[Raw captures and offline replay](native-permit-physical-compare.tar.xz) (SHA-256 `5e16df4616d38075318c634c871cb5bb4d2fc3e4b250009c72d063e1cc5bd322`).
