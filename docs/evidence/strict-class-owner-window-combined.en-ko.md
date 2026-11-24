# Combined finite scheduling verification / 결합 유한 스케줄링 검증

PASS for the pinned one-CPU QEMU configurations / 고정한 단일 CPU QEMU 구성 40개 최초 게스트 합격.

Public source Romanof123/JNOX 4b46f856dfe68f4bb344c7bc3718be0a044ce625. English/Korean input docs/finite-workload-verification.en-ko.md and public build/capture interfaces only through the blind seal. No prior implementation or historical verdict was consulted. WSL ext4 at /home/ywcho/jnox262-combined with per-run Windows mirror. Kernel built using locked release Rust 1.95.0, LLVM 22.1.2, one build job. QEMU 6.2.0 TCG, virt/GICv2, Cortex-A72, 1 vCPU, 128 MiB, default clock. All builds, guest sessions and subsequent host tests serialized. See run.py, raw.py, per-configuration record.json, launch.json and post/environment.log.

결합 소스를 새로 내려받아 최초 원본을 영속 저장한 뒤 비교 소스와 과거 자료를 열었다. first.json은 독립 1차 판정, FIRST.sha256은 실제 최초 원본 전체 해시, SEAL은 두 파일의 봉인값이다. 최초 결합 게스트 40개 모두 기대 터미널에 도달했다. 1회 관측만으로 하드웨어 지연 분포를 추정할 수 없다.

## Strict MLQ / 엄격 MLQ

| Mode | Epoch | Target dispatch | First target counter | High charges | Equal charge | Gaps / excess / entry | Used allowance | Elapsed | Final blocks / odd sum |
|---|---:|---:|---:|---|---:|---|---:|---:|---|
| mlq-admit-plain | 54373 | 4125449 | 4132399 | 22763 / 4973 | 4020297 | 23043 / 20297 / 6950 | 50290 | 4078026 | 7266 / 14880768 |
| mlq-admit-trace | 52820 | 4141501 | 4149768 | 21793 / 4851 | 4030325 | 31712 / 30325 / 8267 | 70304 | 4096948 | 8500 / 17408000 |

At 62,500,000 Hz, two higher demand bounds of 8,000,000, one equal quantum of 4,000,000 and aggregate allowance of 4,000,000 give 24,000,000 counts. The equal worker also has an 8,000,000 total ceiling. Configured deadline B admits, B−1 refuses, both modes. In both refusals all runtime words 6–29 were zero. The admission counters equal high charges + min(equal charge,Q) + gaps + excess + target entry latency. The linked 4,096 bytes are exactly 256 repetitions of 1..16. Independent sums are 34,816, 382,976, 2,048 odds per block and maximum 16. Raw 30-word records for all four cases are in first.json.

추적 게스트에서 실제 디스패치 순서는 0→1→2→3→2→종료였다. ID 2의 타이머 IRQ가 블록 8,499의 내부 인덱스 1,550에서 발생했다. 당시 영수값은 0이었다. 예외 반환 PC 0x40201e2c와 프레임 0x40211e50가 그대로 복구되었다. 블록 8,500 시작 시 ID 2는 16을 읽고 4,096개 샘플을 모두 처리한 뒤 복귀했다. 연결된 코드의 acquire load → 4,096회 루프 → 비교 → 결과 저장 순서 및 원시 프레임을 post/additional.json과 worker.asm에 보존했다. 일반 모드도 IRQ/복귀 PC, 30워드, 실행 코드와 곱을 확인했다.

## EDF and sampled LLF / EDF와 표본 LLF

| Accepted mode | Actual entry ticks, in order | Peak entries / returning changes | A frame / PC restored | A / B absolute deadlines |
|---|---|---|---|---|
| ordinary-edf-supply-cap4-plain | 6303984, 12542606, 18794392, 21917410, 23805387 | 3 / 2 | 0x4020ae10 / 0x40203574 | 37529915 / 50029915 |
| ordinary-edf-supply-cap4-trace | 6300664, 12544081, 18793353, 21914421, 23798136 | 3 / 2 | 0x4020ae10 / 0x40203574 | 37530742 / 50030742 |
| ordinary-llf-supply-cap4-plain | 6301973, 12544719, 18796988, 21916464, 23800602 | 3 / 2 | 0x4020ae10 / 0x40203578 | 37532871 / 50032871 |
| ordinary-llf-supply-cap4-trace | 6317267, 12557281, 18810104, 21925035, 23810955 | 3 / 2 | 0x4020ae10 / 0x40203578 | 37539498 / 50039498 |

Q=6,250,000 counts (100 ms). Each integer window count is constant between event membership boundaries. check.py evaluates both sides of every entry and entry−Q boundary plus domain endpoints. Timer IRQ 30 stayed in each programmed cell 0,1,2. Two one-shot SVC completions may adjoin either turn. The analytic ceiling is 4 entries and 4 returning changes. The final SVC enters but the selected idle frame never returns. Frozen PC lies in WFE/branch park; QEMU ERETs confirm idle→A→service→restored A→B. Each actual completion precedes its absolute deadline and consumes less than its estimate. Independent sum of squares 1²..64² gives 89,440, both finite worker products match the measured block counts modulo 2^64. post/additional.json records products, counts and disassembly for each park.

한도 3의 EDF/LLF 일반·추적 네 게스트 모두 디스패치 전에 거부했다. 선행 상태 네 값이 모두 0이며 JOB/D행·예외가 없었다. 한도 4의 네 게스트에서 A 진척은 예약 서비스 동안 고정되었고 원래 프레임을 복원한 뒤 증가했다. 시간표·상한·실제 전환은 first.json의 게스트별 수치에서 확인할 수 있다.

## Historical controls / 기존 구성

Compared all 28 packaged baseline ELFs at e5ce8a2c49068236a13683bd86846fafa3e8cc82 by freshly booting each image after sealing. Six archived original terminals also match. The registry supplied no original guest captures for the other 22 ELF variants. Those fresh baseline launches live separately under control-reruns/. All 28 terminal UART outcomes and final CPU PCs match. For trace-enabled deadline modes, decision kind, logical slot, owner, next owner, queue, release, exhaustion and miss fields match. Normal legacy supply modes preserve early refusal or 89,440 plus restored A.

28/28 equal .text bytes, section addresses/sizes and final RW PT_LOAD. 0/28 equal complete ELF, .rodata or all PT_LOAD bytes. All 572 differing .rodata bytes decode to Rust source-location line/column fields (src/deadline_jobs.rs, src/multilevel_service.rs, src/ordinary_deadline.rs). See comparison.json for both complete identities/section/load hashes and post/differences.json for byte offsets and decoded locations.

기존 진단 제어에서는 엄격 MLQ 17행·MLFQ 43행을 양쪽 원본 ELF에서 재생했다. 차단 구성에는 생존·수면 중인 두 태스크와 STALL이 남았고 종료 구성에는 생존 태스크가 없었다. 각 한계 구성의 종료 타이머 활성 비트는 0이다. 포화 시 최초 8개 IRQ에서 높은 우선순위 1,2가 교대하며 낮은 작업 서비스는 0, 마지막 높은 작업 종료 직후 낮은 작업이 실행됐다. 지연 구성은 첫 비교값 start+3.25P에서 세 기간을 건너 한 차례 재설정했고 이후 정상 비교값을 복구했다. post/limits.json에 절대 종료와 서비스 수치가 있다. 포화 추적의 최초 비교 게스트 예외 수는 기준 27, 결합 26으로 달랐다. 두 게스트 모두 동일한 최종 PC와 결과에 도달했다.

## Capture coverage and host checks / 수집 범위와 호스트 시험

Original combined strict MLQ cases use the public freeze-enabled helper. The original mlq-blocked-trace uses the public incumbent helper with idle PC and timer-control acquisition, preserving its normal behavior. The generic first collector dumped individual data symbols up to 128 KiB, omitting the large diagnostic MLQ journals in the other historical trace originals. All combined and baseline legacy trace ELFs were subsequently relaunched via the incumbent helper. Their journals are labeled post/trace-repeats. Comparisons based on those journals describe repeats; first captures retain their positions.

프로젝트 테스트 2개 MLQ, 8개 공급/한도, 138개 스케줄러, Python 10개 근거·6개 그래프가 통과했다. 독립 Rust 모형에서 실제 큐 17,730개와 작은 시간 영역의 2,605,536개 IRQ/완료/창 배치를 전수 계산했다. 잘못된 폭·오버플로·누락 수요도 확인했다. 프로젝트 호스트 모듈 두 개를 독립 크레이트로 실행하려던 최초 빌드 진단은 post에 보존했다. 올바른 상위 진입점 tools/scheduling_tests.rs로 다시 빌드해 138개를 통과했다. 한영 릴리스 행렬 및 요구사항 그래프 검사를 실행했다. 행렬 18개 정책·61개 근거·지적 0건이다.

Build for recovered project policy test: rustc --edition=2024 -O --test src/tools/scheduling_tests.rs -o post/scheduling; post/scheduling --test-threads=1. Other invocations and results are in post/commands.json, checker sources and guest launch records. The initial historical-control analysis mistakenly compared the asleep mask of already exited slots. limits.py scores live & asleep; both exits preserve zero live tasks. No guest capture was replaced.

Scope is finite single-core virtual evidence for VER-002/005/023. Deployment acceptance remains with #21 and physical-board qualification with #34. 제품별 워크로드 타이밍과 실물 보드 적합성은 해당 출시 프로필에서 따로 검증한다.

ZIP integrity verification detected that GNU objcopy had rewritten both mlq-plain ELF containers during .rodata inspection. Rebuilt the combined image with the same source, flags and toolchain; SHA256 a291e66032216009c8ea1619aac957b84fab56de31f65f0972988f6bbc4c536f exactly matches the sealed pre-inspection digest. Restored baseline ELF bytes from the registered ZIP. Modified combined copy and rebuild log are retained in post/. Original guest outputs are untouched.

ZIP 해시 검사에서 분석용 objcopy의 변경을 발견했다. 결합 mlq-plain ELF를 재생성해 최초 봉인 해시와 일치함을 확인했고 기준 ELF는 등록 ZIP에서 복구했다. 최초 게스트 출력은 그대로 보존되었다.

[Raw combined evidence / 결합 검증 원본](strict-class-owner-window-combined.zip) · SHA-256 9cf7cc517b806350c229eaf0eb5d5dd015caf495f46ad5731c6f80b003100ad0.
