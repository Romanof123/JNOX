# 두 게스트 CPU의 에너지·기한 기반 배치

[English](m3-power-placement.en.md) · [이슈 #58](https://github.com/Romanof123/JNOX/issues/58)

CHG-027 · POL-017 · IMP-025. 이 인수 절차는 EVD-025에 수집할 증거를 정의한다. QEMU virt의 Cortex-A72 vCPU 두 개 중 선택된 CPU가 스택을 사용해 같은 합산 작업을 4,096회 반복한다. 합은 8,390,656이다. OPP 식별자, 처리 용량, 열 제한, 기상 비용과 에너지는 모델 입력이다. MPIDR, 스택 포인터, 계산 결과와 범용 카운터 시각은 게스트 실행에서 얻는다.

## 작업과 예상 결정

모든 요청은 모델 시각 0에 시작하며 작업량은 12, 최소 용량은 1, 기한은 6, 보호할 기한 여유는 1이다. 두 CPU가 사용 가능하고 예약 용량은 0이다. 기본 CPU 마스크는 3이다. 유효 용량은 명목 용량과 열 제한 중 작은 값에서 공유 부하와 예약 용량을 빼고 0을 하한으로 삼는다. 실행 틱은 작업량을 유효 용량으로 나눈 올림값이다. 기상과 실행이 틱 5까지 끝나는 후보 중 모델 에너지(기상 에너지 + 작업량 × 단위 에너지)가 가장 작은 점을 고른다. 동률이면 CPU, OPP 순으로 작은 번호가 우선한다.

| CPU / OPP | 용량(작업량/틱) | 단위 작업 에너지 | 기상 에너지 | 기상 틱 |
|---|---:|---:|---:|---:|
| 0 / 0 | 3 | 1 | 1 | 1 |
| 1 / 1 | 6 | 3 | 2 | 1 |

| 행 | 변경한 입력 | 예상 결정 | 실행 / 기상 / 기한 여유 / 에너지(모델) |
|---:|---|---|---|
| 0 기본 | 열 제한 3, 6 | 모델 선택 CPU0/OPP0 | 4 / 1 / 1 / 13 |
| 1 열 제한 | CPU0 열 제한 2 | 모델 선택 CPU1/OPP1 | 2 / 1 / 3 / 38 |
| 2 공유 | CPU0 공유 부하 1 | 모델 선택 CPU1/OPP1 | 2 / 1 / 3 / 38 |
| 3 기한 | 두 열 제한 모두 1 | 기한 위반 예측, 배치 없음 | — |
| 4 대체 | 빈 표와 신뢰된 CPU1/OPP1 제공 | 실행 가능한 대체 CPU1/OPP1 | 2 / 1 / 3 / 38 |
| 5 유휴 | CPU 마스크 0 | 유휴, 배치 없음 | — |

행 2에서 CPU0은 배치 전에 배경 합산을 한 번 끝낸다. CPU1은 전경 합산 중간에 게이트 값 2를 게시하고 기다린다. CPU0은 두 번째 합산을 끝내고 값 3을 게시한다. CPU1의 완료를 관측한 CPU0은 세 번째 합산을 수행한다. acquire/release 연산이 두 번째 배경 합산을 전경 실행 구간 안에 배치한다. 측정된 응답 시간에는 대기 시간과 첫 번째 배경 합산이 포함된다. 용량 감소를 나타내는 모델 입력값은 1이다.

## 재현과 검증

Linux 저장소 루트에서 실행마다 새 캡처 경로를 지정한다. 관측기는 vCPU 두 개, GICv2, 멀티스레드 TCG를 선택한다.
```sh
(cd kernel && cargo build --locked --release --features power-bench -j1)
elf=kernel/target/aarch64-unknown-none/release/jnox-kernel
wrap=$(mktemp)
printf '#!/bin/sh\nexec python3 "%s/tools/power_capture.py" "$@"\n' "$PWD" > "$wrap"
chmod +x "$wrap"
POWER_CAPTURE_DIR="$PWD/power-capture-01" python3 tools/qemu_boot.py --profile power --kernel "$elf" --qemu "$wrap"
```

ELF, 리비전, SHA-256, 도구 버전, 실행 인수, UART 바이트와 QMP 덤프를 함께 보존한다. 5초 안에 다음 기록이 완성되고 100ms 동안 추가 출력이 없으며 호스트가 종료하는지 확인한다.
```text
JNOX M1 BOOT
JNOX M1 READY
POWER BASE
POWER THERMAL
POWER SHARED
POWER DEADLINE MISS
POWER FALLBACK
POWER IDLE
POWER PASS
```

동일한 ELF에 `aarch64-linux-gnu-nm -S`를 적용해 `power_witness`의 위치와 크기를 구한다. `launch.json`, `qmp.json`, `audit.bin`, `int.trace`, `uart.bin`, `exit.json`을 독립적으로 확인한다. QMP는 실행 중인 게스트와 CPU 인덱스 0, 1을 보고해야 한다. 물리 메모리 덤프의 주소와 크기를 심볼과 대조한다. 리틀엔디언 64비트 워드로 읽는다. 헤더 8개 워드는 순서대로 ABI 버전 1, 게시된 행 수 6, 완료 플래그 1, CNTFRQ, 모델 틱 길이(CNTFRQ / 10의 정수 몫), 반복 횟수 4096, 기대 합 8390656, 원격 작업 수 3이다. 행 i는 워드 8 + 34 × i에서 시작한다. 유효 데이터 212워드는 1,696바이트이며 64바이트 정렬 객체 끝에는 패딩이 있다.

```text
 0 case                 1 decision             2 model_points        3 allowed_mask
 4 work                 5 min_capacity         6 deadline_ticks      7 deadline_reserve
 8 cpu0_thermal         9 cpu0_shared         10 cpu1_thermal       11 selected_cpu
12 opp                 13 capacity            14 run_ticks          15 wake_ticks
16 model_margin        17 model_energy        18 release            19 dispatch
20 work_start          21 work_end            22 observed_complete  23 deadline_counter
24 measured_margin     25 mpidr               26 sp                 27 result
28 load_start          29 load_end            30 load_batches       31 load_result
32 dispatch_to_work    33 response
```

결정 코드 1~4는 차례로 모델 선택, 대체, 기한 위반 예측, 유휴다. 행 3과 5에서 selected_cpu는 UINT64_MAX이며 배치·실행·결과·실측 시간 필드는 0이다. 모든 행에서 deadline_counter는 release + 6 × 헤더의 틱 길이다. 실행된 작업은 MPIDR의 CPU affinity 비트, 0이 아닌 정렬된 SP, 동일한 합과 release ≤ dispatch ≤ work_start < work_end ≤ observed_complete를 검증한다. dispatch_to_work는 work_start − dispatch, response는 observed_complete − release, 부호 있는 measured_margin은 deadline_counter − observed_complete로 다시 계산한다. 필드 24는 차이를 2⁶⁴로 나눈 나머지로 저장한다. 카운터 차이를 CNTFRQ로 나누면 초 단위가 된다. 행 2는 배경 합산 세 번, 같은 합, 전경 실행을 포함하는 배경 구간을 확인한다. 나머지 행의 배경 필드는 0이다.

명령 실행을 독립적으로 확인할 때는 같은 ELF의 `power_work_loop`, `power_work_end` 주소와 역어셈블리를 확인한다. 별도 계측 실행에서 매 호출의 루프 진입 시 vCPU/스레드, PC, SP를 기록한다. 기대 호출 수는 CPU0 전경 한 번, CPU1 전경 세 번, CPU0 배경 세 번이다. PC가 루프 심볼 범위 안에 있고 각 CPU의 스택이 정렬되었는지 확인한다. 계측 실행은 제어 흐름에, 중단 없는 실행은 시간 측정에 사용한다. QMP CPU 목록과 예외 로그만으로는 일반 루프 PC를 관측할 수 없다.

## 실보드 보정

보드 리비전, 펌웨어, 거버너, 냉각, 전원, 주변 온도와 작업 식별자를 기록한다. 허용된 각 CPU/OPP 및 유휴 상태에서 실제 OPP와 온도를 확인하고 같은 작업을 반복하면서 릴리스, 기상 요청, 첫 실행, 완료 시각을 기록한다. 외부 전력계나 션트 측정기의 시간축을 이벤트와 동기화한다. 전압·전류 샘플 시각, 적분 구간, 유휴 기준선을 명시하고 전력을 적분한다. 통제된 경쟁 부하와 실제 열 제한 조건에서 반복한다. 원본 파형, 보정 불확도, 카운터 주파수, 측정 용량, 기상 지연, 에너지, 응답 여유, 관측된 기한 위반을 보존하고 보드 프로파일이 명시한 기준과 대조한다. 실제 에너지·기상 지연·OPP 전환·온도·기한 동작 판정에는 이 측정이 필요하다.
