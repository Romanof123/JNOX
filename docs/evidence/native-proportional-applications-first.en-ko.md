# Independent native proportional firsts / 네이티브 비례 스케줄 최초 검증

Source f107b60fc1cb5bc83767ee98299d7042a3103b7a, contract 29a4785070c64c5698fc1b44445fdcd7d7812da2. Seven ordinary builds and seven cold QEMU boots succeeded serially on WSL ext4. No retries or artificial IRQ delay. All six policy journals have zero drops through final exit. 1 vCPU Cortex-A72, GICv2, TCG, default 62.5 MHz counter, quantum 625000. 각 최초 실행을 그대로 보존했고 정책 실행 6건의 종료까지 누락이 0건이었다.

| Run | Events / selections | PPI / external EL0 records | Raw physical ticks in slot order |
|---|---:|---:|---|
| seed1 | 451 / 450 | 447 / 900 | [43598684, 118154185, 120245665] |
| seed2 | 448 / 447 | 444 / 894 | [43409905, 117026333, 120482660] |
| stable | 448 / 447 | 444 / 894 | [43301488, 117701519, 120212032] |
| dynlot | 83 / 76 | 68 / 164 | [59434, 43281430] |
| dynstride | 84 / 77 | 69 / 166 | [59834, 43871076] |
| solo | 71 / 70 | 69 / 140 | [44012239] |
| legacy | — / — | — / — | default boot, exit 237 |

Seed 1 totals 11/14/23, minima 5/3/11. Seed 2 totals 8/17/23, minima 1/7/10. All 25 sliding windows and 48 full closures per seed passed. 실제 배정 횟수와 누적 카운터 서비스는 별도로 기록했다.

Stable stride all 48 exact prefixes passed. M=634686, maximum normalized spread=634686, maximum absolute errors in ticks by owner ['3808116/7', '3124748/7', '5053932/7']. First 48 totals [4412618, 8809860, 16996099]. 모든 접두 구간과 종료까지 패스 변화를 재생했다.

CRC 524288 blocks / 134217728 bytes / CRC32 2124873618 / exit 237. Histogram 2097152 blocks / 536870912 bytes / bins [2097152, 16777216, 58720256, 117440512, 146800640, 117440512, 58720256, 16777216, 2097152] / exit 70. Energy 4194304 blocks / 1073741824 samples / 5864240971776 / exit 179. The reporter completed three 4096-byte rounds with 256 hits each, 768 final, and exit 53. Private output pages, stacks, sampled saved SP, disassembled PC, per-slot roots and every externally traced EL0 exception/return matched for the six policy runs. EL0 SP/PSTATE come from the saved frames; QEMU independently reports ELR and return PCs. 각 앱 사설 결과를 독립 산술과 비교했다.

Dynamic observations (ticks) / 실제 수면 관측

dynlot: three voluntary yields and real 40 ms sleeps. Wake lateness [3994, 4833, 4632]; CRC lease ticks while blocked [2471419, 2498874, 2495826]; shortest positive lease 3510. CRC sampled PCs and the final product establish useful worker execution; intermediate RAM progress counters were not sampled at each wake.

dynstride: three voluntary yields and real 40 ms sleeps. Wake lateness [13342, 3850, 5178]; CRC lease ticks while blocked [2478741, 2498971, 2497594]; shortest positive lease 4398. CRC sampled PCs and the final product establish useful worker execution; intermediate RAM progress counters were not sampled at each wake.

Both public admission and raw build refused [24,48), owner A, 5 < 6, exit codes 2 and 101. No executable was produced. Policy host controls 11/11 passed, including u32::MAX and +1, u64::MAX full product and wrong-owner refusal with unchanged state. The largest possible positive product fits in 84 bits. The selected owner starts at zero, making u128 addition overflow unreachable through admitted dispatch/stop inputs. Host controls did not force an internal u128-overflow state. 정상 설정으로 도달 불가능한 내부 덧셈 오버플로의 인위적 시험은 실시하지 않았다.

First offline reader setup encountered ModuleNotFoundError (elftools); both lottery captures and all other original archives scored unchanged using the installed dependency. An earlier shell quoting error selected a nonexistent capture directory and produced score-.json. Neither invocation launched a guest. 이전 #79 실패의 분기와 피연산자는 식별되지 않았다. 기존 원인, 실기 시간 보증 및 외부 제품 위험 수용은 이번 판정의 대상이 아니다. Historical #79 remains unattributed; the initial historical failure and Functional Safety/product authority retain their dispositions.

The raw packet includes session hashes, exact QEMU launch, private RAM, kernel/ELFs, disassemblies, UART, external exceptions, first-build logs, replay and independent interval/product reports. Captured physical elapsed service includes kernel work within anchored nonblocking brackets.
