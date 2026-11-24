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

## Integrated source / 통합 소스

At `6a336318e265785b5c0d8df3b5a8649632520919`, the first three fresh boots passed in order (exact LO, exact HI, one over HI). A prelaunch protocol fixed these expectations. Exact LO retained owners 0,1,0. Both HI cases scheduled 0,1,1,1,3,0 and finished generation (0,2) with eight samples and sum 36. The following raw ticks use the guest's 62.5 MHz counter.

통합 소스의 최초 세 부팅은 exact LO, exact HI, one over HI 순서로 모두 통과했다. 기동 전에 예상 결과를 고정했다. 정확히 LO인 경우의 슬롯 소유 순서는 0,1,0이었고 두 HI 사례는 0,1,1,1,3,0이었다. 두 HI 사례에서 세대 (0,2)가 표본 8개, 합 36을 기록했다. 아래는 게스트 62.5 MHz 카운터의 틱이다.

| Case / 사례 | Publication / 작업 공개 | SVC | Original b6 / 기존 b6 | Idle IRQ / 유휴 IRQ | Retirement / 반납 |
|---|---:|---:|---:|---:|---:|
| Exact HI | 15730076 | 15738150 | 18821936 | 18832977 | 18840714 |
| One over HI | 15713977 | 15719712 | 18816933 | 18828030 | 18838400 |

The same-process exception histories in both HI cases recorded SVC continuation 0x402023c8, return to idle at 0x402022f4, then idle IRQ ELR 0x402022f8. The last IRQ reread the original task count, sum and tick before the single retirement. Four mutations per HI capture were rejected. The integrated decoder reproduced 1/3 on the archived continuous cohort and 3/3 on the archived finite cohort. All three integrated AMC entry points, PT_LOAD layouts and loaded bytes matched the corresponding 420eaaa images. The full ELF hashes differed. Comparison builds for ordinary PI, ordinary ceiling and legacy AMC bench matched the entry and load segments of fresh #157 builds. Pi 4 directed-wake matched the entire #156 ELF. These four compatibility comparisons added zero boots. Six adjacent invalid configurations exited 101. An earlier host build supplied an empty anchor variable and exited 101 before guest launch. Removing the variable restored the intended configuration.

두 HI 사례 모두 같은 프로세스의 예외 로그에서 SVC 연속 주소 0x402023c8, 유휴 복귀 0x402022f4, 유휴 IRQ의 ELR 0x402022f8을 확인했다. 최종 IRQ에서 기존 작업의 표본 수·합·틱을 다시 읽은 후 한 번 반납했다. 각 HI 사례의 변형 입력 네 건은 모두 거부됐다. 통합 디코더는 기존 연속 코호트 1/3 및 유한 코호트 3/3을 재현했다. 통합 AMC 세 이미지의 진입점·적재 배치·적재 바이트가 420eaaa의 각 이미지와 일치했고 전체 ELF 해시는 달랐다. 일반 PI·천장·기존 AMC 벤치의 진입점과 적재 영역은 #157 소스에서 재빌드한 결과와 같았고 Pi 4 directed-wake는 #156 전체 ELF와 일치했다. 이 네 호환성 비교의 추가 부팅은 0회다. 잘못 조합한 설정 여섯 건은 모두 101로 종료됐다. 첫 호스트 빌드에서 빈 앵커 변수를 전달해 게스트 실행 전 101로 중단됐고 변수를 제거한 뒤 의도한 설정으로 빌드했다.

Extract the archive, run `python independent-observer.py` at its root, then run the integrated source's `tools/ordinary_amc_replay.py integrated/CASE DIGEST` using each integrated/CASE.session digest and pyelftools 0.32. The archive contains unchanged guest and same-process observer bytes, per-case frozen ELF and manifests, protocol, comparison hashes, build logs and path-normalization hashes. Guest launches used native WSL ext4, QEMU 6.2.0, single Cortex-A72 virt/GICv2 and 128 MiB. ZIP SHA-256 is `66140607727de076d5f78172556ae30a2bfb0ae45eaf3e474e641b27a8500cae`. [Integrated captures and replay inputs / 통합 원시 자료와 재생 입력](ordinary-amc-integrated.zip)
