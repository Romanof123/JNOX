# Fixed-input native EDF first results / 고정 입력 EDF 최초 실측

Public source 424a63fbce01189c9be054107e4e6e42ec5ad965, accepted main eb73c63a925aa9a17982bdc1a0d79b768ed03c5f. The diff changes batch.c and two documents. The kernel, Rust histogram and urgent C source match the accepted loader tree. The normal operator packaged the same three ELFs into image 5eb35247e356920fca87f81d529b0c64ec61e02de3e41a4fe6e7f854912b7b8e. Each manifest records a clean tracked source tree. Both first boots used one QEMU 6.2 Cortex-A72 TCG CPU, virt/GICv2, 128 MiB and the stock clock. No guest was restarted or retuned. A preliminary build in WSL /tmp disappeared prior to any guest boot; the retained build and all six first guest positions used WSL ext4 in the home directory.

Both cold boots ran A → C → A → B. Grid 2 retained running A. All six principal job exits preceded their own physical deadlines. Both runs returned 17/29/43. A reduced 32,768 complete 4 KiB blocks (128 MiB), sum 17,112,760,320. B completed 128 blocks with sixteen bins of 32,768. C completed eight blocks and counted 2,048 exceedances. QMP read each product from the independent process's physical backing. Zero skipped grids and zero dropped rows.

Q = 6,250,000 ticks; frequency 62,500,000 Hz. Triples below are A/B/C. Each elapsed value is actual exit minus the common epoch S. Each margin is that slot's deadline minus actual exit. Absolute deadlines are S + 8Q/12Q/4Q.

| Boot | S | Exit elapsed (ticks) | Margin (ticks) | Grid-1 IRQ lateness |
|---|---:|---|---|---:|
| Plain | 6,934,125 | 15,912,356 / 16,020,251 / 6,312,613 | 34,087,644 / 58,979,749 / 18,687,387 | 15,487 |
| External | 6,877,534 | 15,562,787 / 15,660,520 / 6,340,375 | 34,437,213 / 59,339,480 / 18,659,625 | 28,328 |

Both journals saved and restored PC 0x4001e8, SP_EL0 0x1010f40 and A's frame 0x402141c0. The PC is the inner byte load ldrb w10,[x9,x8] in batch_reduce. The fixed loop stores the sum before the completed count and calls only batch_reduce. Saved X21 minus one gives the published block count at this instruction: 12,937 plain and 13,120 traced. Later QMP reads observed 13,574 and 13,571, respectively, after C had published eight, then further progress to 32,768. These are asynchronous host reads; exact interrupted counts are derived from the captured saved register and disassembly.

In the external trace, the first EL0 IRQ has ELR 0x4001e8, X21=0x3341 and LR 0x400410 in A's fixed-mode caller. CPU0 acknowledged GIC IRQ 30. ERET entered C at 0x400000. Its exit SVC saved 0x400020. During the return path the CPU dump at 0x40202354, immediately after msr sp_el0,x9, has X9=0x1010f40 and X10=0x4001e8. X15=0x000100004025a000 matches ASID 1 and A's page-table root. QEMU then emitted the actual ERET to EL0 PC 0x4001e8. Native scheduling records, external exception order, and architectural SP evidence are separately retained. The tested frame fields are PC/SP and native frame identity. Full register-file equality was not measured.

With the same batch ELF, --blocks 1 produced (1, 522240), exit 17. Both --blocks 65537 and --blocks 2x produced (0,0), exit 105. No extras selected the original timed code with clock read before initialization, a check after each block, 65,536 cap and original errors 101–104. It produced (17,608, 9,195,601,920), exit 17 at S+10,072,692 ticks (161.163072 ms); successful exit also requires the client clock's 160 ms threshold. The historical #299 plain reduction interruption and external clock-helper interruption remain the original findings for #298. This evidence qualifies only the fixed N=32768 configuration on the named emulator.

일반 및 외부 추적 최초 콜드 부팅에서 A→C→A→B를 관측했다. 격자 1의 실제 타이머 인터럽트가 A의 바이트 읽기(0x4001e8)를 선점했다. C 종료 뒤 같은 PC·SP(0x1010f40)·A 프레임으로 복귀했다. 외부 추적에는 GIC IRQ 30 승인, C 진입 및 종료, SP_EL0 쓰기 직후의 레지스터와 A의 EL0 복귀가 남아 있다. 선점 시점 완료량은 각각 12,937 및 13,120블록이며 이후 32,768블록, 합 17,112,760,320으로 끝났다. B는 128블록과 16개 빈 각각 32,768, C는 8블록과 초과값 2,048을 기록했다. 종료 코드는 17/29/43이며 표의 개별 마감을 모두 충족했다. 누락 격자·손실 행은 0이다.

동일 ELF에서 1블록은 522,240과 종료 17, 잘못된 65537 및 2x는 진행량·합 모두 0과 종료 105였다. 무인수 기존 시간 모드는 161.163072 ms 경과에 17,608블록과 정상 종료를 기록했다. 기존 #299의 외부 시계 함수 선점은 종전 판정대로 유지한다. 이번 판정은 명시한 에뮬레이터와 고정 입력에 한정된다.

Offline replay: run python3 score.py and compare output with scores.json; verify SHA256SUMS from the packet root. elf contains individual ELFs, disassemblies, symbol tables, package manifests and images. runs contains untouched UART, QMP private memory, saved-frame samples and the filtered QEMU exception/register trace. build.sh and capture.py retain the actual local prerequisites and invocation.
