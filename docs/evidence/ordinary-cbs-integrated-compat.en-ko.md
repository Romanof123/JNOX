# Legacy CBS compatibility at the integrated reservation candidate / 통합 예약 후보의 기존 CBS 호환성

The two original normal workloads produced different outcomes on `259aaebb4138cde18d1851fc567bc09632d1268c`. EVD-006 failed on its first cold boot. EVD-029 passed on its first cold boot. These separate compatibility runs preserve the original eight reservation positions and their guards.

| First normal launch | Observed UART and control flow | Outcome |
|---|---|---|
| Candidate cbs-bench | 83 bytes through A FIRST, B FIRST, IDLE, A RESUMED, then CBS FAIL. Four IRQs. The fourth interrupted A at 0x402012c8. No ERET to B followed. | Observer 1 |
| Candidate cbs-wake-bench | Original terminal UART, all 34 rows and all 78 ordered exception/return PCs match the EVD-029 reference on their comparable fields. | Observer 0 |
| Distinct main control cbs-bench, 12a98a9178903798b6b90cced17bd785d219eedc | Original 112 UART bytes and complete EVD-006 PC sequence. Five IRQs, B resumed, timer stopped. | Observer 0 |

In EVD-006, the candidate's recorded PC events match the reference prefix through IRQ four. Several guards can lead to the same failure marker; the UART and exception trace do not identify which one fired. The first failure was retained with no replacement candidate boot. In EVD-029 the independent comparison matched release masks, budgets, deadlines, ownership, dispatch and interrupt frames and PCs. Five SVCs occurred at slots 0, 4, 8, 14 and 22. B received nine slots and idle received 17. Counter values are specific to each boot. All 34 IRQs fell inside their original 3,125,000-tick bounds. Each run used Linux Rust 1.95.0, locked release builds with one job, one emulated A72, virt/GICv2, TCG and 128 MiB. The wake observer kept its accepted 15-second host allowance.

후보의 기본 CBS는 최초 부팅에서 A 재개 후 네 번째 IRQ까지 기존 기록과 일치했으나 B로 복귀하기 전에 CBS FAIL을 출력했다. 공통 실패 표식만으로 원인이 된 검사를 특정할 수 없다. 별도 main 대조 빌드는 최초 부팅에서 다섯 IRQ와 정상 종료를 확인했다. 후보의 차단·깨우기 CBS는 최초 부팅에서 34개 슬롯의 정책 상태 및 78개 예외·복귀 주소가 기존 자료와 일치했다. 이 호환성 시험은 기존 예약 시험 여덟 건과 별개다.

Extract the archive, then run `python verify.py /path/to/JNOX/docs/evidence` from the extracted root. It compares the retained observations to EVD-006 and EVD-029 and emits the per-run results. ZIP SHA-256 `f1e76bcfb7f46d94ae23ea1327e9b2c884a9a8aa152e37a7d5171e6d044c7883`. [Original attempts, ELF files and independent comparison / 최초 시도·ELF·독립 비교](ordinary-cbs-integrated-compat.zip)
