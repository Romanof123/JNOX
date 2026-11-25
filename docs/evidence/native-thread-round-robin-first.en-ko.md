# Independent first boots / 독립 최초 실행

Public source 9df9437b626da3bcaa4151f3db1370e7db24ae13 was fetched independently into WSL ext4. Stock QEMU 6.2.0 virt/GICv2, TCG, one A72, 128 MiB, default clock. CLK 62,500,000 Hz, Q 625,000 ticks. All 15 serial cold boots reached the expected status. Original main stream digest 17e27adc8a3a1d0532a87ad576ef345e6365f38362682bb2e1f6f4eb59d741ec. Freeze, commands, source, disassembly, manifests, raw RAM, UART, external exception logs, and offline scorer are included. Terminal QMP stops occurred after exit. No retries or injected latency.

동일 ELF의 최초 RR 일반/외부추적, 협력형 일반/외부추적을 차례로 실행했다. 독립 WSL ext4 환경의 QEMU 기본 클록에서 15회 모두 예상 종료값을 얻었다. 원본, 실행 명령, 전체 UART/예외 기록, 종료 후 메모리 및 판독기를 보존했다.

| Cases / 사례 | Observations / 관측 |
|---|---|
| RR plain, traced / RR 일반·추적 | 22 involuntary turns each, 0 dropped. Both threads completed 16,384 blocks each, checksum 8,556,380,160, sixteen bins each 4,194,304, descriptor 0x402000, exit 17. Plain service 7,477,996 / 14,621,674 ticks. Traced 7,478,356 / 14,514,788 ticks. |
| EL0 trace / 실제 복귀 | 22 PPI30 acknowledgements. First main interrupt 0x400328 SP 0x1010f60, entering new worker at 0x400024. Next worker interrupt 0x400384 SP 0x1008fc0 restored main's 0x400328 and SP. Third interrupt restored worker's 0x400384 and SP. External ERET PCs match all selected PCs. Same TTBR, distinct aligned stacks and errno cells. |
| Timing / 시간 | RR trace expiry lateness 3,715–10,264 ticks (0.059–0.164 ms). The first interrupted accounting segment was 588,036 ticks since CREATE returned on the original turn. All RR charges independently sum entry minus preceding pre-restore per owner. Nonblocking syscall handler time, scheduler gaps and idle excluded. Pre-restore bounds the beginning of user execution. |
| Cooperative / 협력형 | Both firsts completed identical products and exit 17. CREATE returned to main. First worker dispatch followed main's blocking join after its reduction. No EL0 timer switches. |
| Early exit / 조기 종료 | 1 worker block, main 16,384, exit 17. 6,799,365 ticks after worker exit with zero further timer switches. |
| Absolute sleep and join / 절대 수면·조인 | Both blocked. One idle PPI30, resumed worker, pointer transferred, exit 17. SDK target 23,257,040 ns, next observed clock 23,669,504 ns. |
| Wake/creation / 깨움·생성 | At 3 ms sleep the wake IRQ arrived 451,514 ticks before the incumbent expiry and retained the incumbent. At 25 ms sleep it arrived 928,031 ticks beyond expiry and rotated. Both modes explicitly yielded before the worker slept. Late create occurred 6,753,638 ticks after logical expiry and the next IRQ rotated. |
| Errno and reuse / 오류·재사용 | Across 22 RR preemptions main kept 22, worker 38. Reuse and cooperative sensor cases validated both full calibration arrays, handles 3/5, stale return 3, join pointers, exit 17. |
| Compatibility / 호환 | Ordinary process RR returned 12 from each separately mapped prime counter; the C program validates 78,498 primes. Rust cooperative and RR exited 29, terminal RAM verified 256 squares and sum 5,559,680. Neither Rust workload received an EL0 timer interrupt. 7/7 host thread-policy tests, including ties, late samples and arithmetic failures. |

The RAM snapshots are terminal. Intermediate per-block progress follows from finite monotonic counters, interrupted productive PCs, resumed execution and final output; intermediate counter values were not independently sampled on each turn. QEMU external tracing supplies actual EL0 return PCs; stack and errno values are from guest rows. No exhaustive asynchronous SIMD, PSTATE or general-register canary, exact hardware ERET timestamp, physical-board timing, multi-core certification or guest simultaneous-tick measurement. Exact-tie and overflow coverage is host-model only. No independent CLI rejection matrix in this packet.

RAM은 종료 후 한 번씩 확보했다. 블록별 누적 진척은 실제 계산 루프 중단·복귀와 최종 결과로 확인했으며, 매 전환의 중간 카운터 수치는 직접 측정하지 않았다. 외부 예외 추적은 실제 EL0 복귀 PC를, 게스트 기록은 SP와 errno 위치를 제공한다. 전체 SIMD·PSTATE·범용 레지스터 패턴, 실제 보드·멀티코어, ERET 시각을 측정하지 않았다. 동시 틱·산술 한계는 호스트 모델에서 검증했다. CLI 거절 조합은 이 패킷의 독립 검증 범위 밖이다.
