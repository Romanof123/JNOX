# Ordinary Rust AMC finite completion · first boots / 일반 Rust AMC 유한 작업 최초 부팅

All three first boots passed on one emulated AArch64 Cortex-A72. Exact LO retained its v1 continuous workload, and its slot owners were 0, 1, 0. Each v2 transition ran owners 0, 1, 1, 1, 3, 0 and published the finite result from generation (0,2).

단일 AArch64 Cortex-A72 에뮬레이션에서 세 사례 모두 첫 부팅에 통과했다. Exact LO는 v1 연속 작업과 슬롯 소유 순서 0, 1, 0을 유지했다. 두 v2 전이 사례의 순서는 0, 1, 1, 1, 3, 0이었다. 두 경우 모두 새 세대 (0,2)가 여덟 표본의 합 36을 공개했다.

| Counter observation / 카운터 관측 (62.5 MHz) | Exact HI | One over HI |
|---|---:|---:|
| Transition to first resumed work, x−b2 / 전이 후 첫 작업 | 24,247 ticks (387.952 µs) | 31,723 ticks (507.568 µs) |
| Third HI grant retired / HI 세 번째 슬롯 반납 | 12,600,410 (complete) | 12,560,716 (one denied) |
| b5 | 15,714,829 | 15,673,714 |
| Survivor retired / 생존 작업 반납 | 15,729,987 | 15,686,257 |
| Final task publication / 최종 작업 공개 | 15,745,243 | 15,697,704 |
| Post-publication SVC upper bound / 공개 후 SVC 상한 | 15,755,050 | 15,705,381 |
| Original b6 / 기존 b6 | 18,839,829 | 18,798,714 |
| Idle IRQ / 유휴 IRQ | 18,853,170 | 18,809,130 |
| Single final policy retirement / 마지막 정책 반납 1회 | 18,859,992 | 18,815,210 |

Both generations completed before their original b6. The linked SVC and private idle return matched the same QEMU process's exception trace. IRQ 30 interrupted idle at the original compare and observed the same count, sum and task tick. The policy retired the sole grant after that IRQ. Discarded LO jobs received zero service. Each transition rejected four host mutations covering an SVC exactly at b6, changed frozen sum, wrong idle return and duplicate SVC.

두 세대 모두 기존 b6 전에 완료했다. 같은 QEMU 프로세스의 예외 추적에서 ELF의 SVC와 전용 유휴 프레임 복귀를 대조했다. 기존 비교값에 대응하는 IRQ 30에서 개수·합·작업 틱이 그대로였고, 정책은 IRQ 이후 해당 슬롯을 한 번 반납했다. 폐기한 LO 작업은 추가 서비스를 받지 않았다. 두 사례 모두 b6 시각의 SVC, 변경된 합, 잘못된 유휴 복귀, 중복 SVC를 각각 거부했다.

The original continuous-worker archive reproduced its original 1/3 with both decoder revisions. Exact HI and one over HI were late by 10,507 and 9,225 ticks. Each new case had one cold launch, with Windows Rust 1.95.0 locked release builds and native WSL Ubuntu 22.04 ext4 QEMU 6.2 virt, GICv2/CNTP, TCG and 128 MiB. The trace exposes first and final sample publications. Product interference, board timing and worst-case bounds require separate measurements.

기존 연속 작업 자료는 두 디코더에서 모두 원래의 1/3 판정을 재현했다. Exact HI와 One over HI의 초과량은 각각 10,507틱과 9,225틱이다. 새 사례마다 독립된 단일 작업 빌드 후 콜드 부팅을 한 번 실행했다. 기록은 첫 표본과 마지막 표본을 제공한다. 제품 간섭·실제 보드 타이밍·최악시간 경계는 별도 측정이 필요하다.

[Metrics and hashes / 측정값과 해시](ordinary-amc-finite-first.json) · [Captures and replay / 캡처와 재검증](ordinary-amc-finite-first.zip) · [Original first cohort / 기존 최초 코호트](ordinary-amc-first.en.md) · [Frozen design / 고정 설계](../ordinary-amc-plan.en.md)
