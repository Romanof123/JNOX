# 복권 스케줄링과 실행량 기반 스트라이드

[English](m4-proportional.en.md) · [변경 이슈 #54](https://github.com/Romanof123/JNOX/issues/54)

CHG-023 · POL-010 · REQ-002/005/023 → DES-003/012 → IMP-021 → VER-002/005/023. Cortex-A72 한 개와 GICv2에서 전용 스택을 가진 A·B·C의 가중치를 1·2·4로 둔다. 복권 사례는 초당 100회 기준 물리 타이머 허용을 시드당 32회 수행한다. SplitMix64의 초기 상태는 1과 2다. 활성 티켓 합으로 2^64를 나눈 나머지보다 작은 원시 난수는 버리고, 다음 적합한 값의 나머지로 추첨한다. 세 작업이 준비 상태이면 0은 A, 1~2는 B, 3~6은 C를 선택한다. 구간 밖 추첨은 거부한다. 시드별 선택 횟수와 32 × 가중치 / 7의 차이는 작업마다 최대 8회다. 호스트에서 시드 1~8에 각각 4,096번 추첨하고, 시드별·작업별 비율 오차를 3%포인트까지 허용한다.

스트라이드는 패스가 최소인 준비 작업을 고르고 동률이면 작은 ID가 우선한다. 활성 작업의 최소 패스를 함께 빼서 상대 순서를 보존한다. 초당 25틱 기준 실제 경과한 정수 틱에 720,720 / 가중치를 곱해 계상한다. A는 1틱 후 양보하고 B·C는 2틱을 쓴다. A·B만 준비된 첫 여덟 번은 A,B 네 차례 반복으로 서비스가 4·8·0이어야 한다. C를 기존 작업의 최소 패스에서 합류시키면 이후 A,B,C,C가 두 번 반복돼 2·4·8틱이 추가된다. 합류 전후 패스, 정규화 및 반환 후 사용량을 기록한다.

kernel에서 `cargo build --release --locked --features proportional-bench -j1`로 빌드한다. 저장소 루트에서 `python3 tools/qemu_boot.py --profile proportional --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel`를 실행한다. 첫 관측은 새 PROPORTIONAL_CAPTURE_DIR에 저장하고 `python3 -B tools/proportional_capture.py "$@"`를 호출하는 Linux 래퍼를 --qemu에 준다. 부팅 후 첫 세 표식은 LOTTERY SEED A, LOTTERY SEED B, STRIDE SERVICE JOIN 순이다.

마지막 대조 시험에서 CPU 중심 작업 하나에 2틱을 부여하고 3틱 이상이 될 때까지 IRQ를 마스킹한다. prop_delayed_work에서 마스킹을 풀면 PPI 30이 들어와야 한다. 가중치 1이므로 관측한 모든 정수 틱(최소 3)을 계상하고 패스가 720,720 × 실제 틱만큼 전진해야 한다. PROPORTIONAL PASS 직전에 STRIDE DELAY CHARGED를 출력한다. 일반 복권 시험에서는 선택 횟수와 실제 서비스 합계를 별도로 유지한다. 일반 사례의 허용 시간을 벗어난 실행은 측정 행을 남기고 실패로 판정한다.

ELF 심볼 proportional_witness의 크기는 10,432바이트다. 앞의 리틀엔디언 u64 다섯 개는 행 수·두 틱 길이·두 시드이고 다음 u32 여섯 개는 합류 전후 패스다. 81개의 128바이트 행마다 u64 열네 개(ELR·문맥 주소·시작·정지·타이머 만기·진행량·정규화·선택 시 패스 셋·반환 후 패스 셋·관측한 정수 틱)가 있고, 다음 16바이트는 사례·슬롯·소유자·준비 마스크·인터럽트 ID·추첨값·티켓 합·구간 시작·배타적 끝·계상 적용 여부·예약 여섯 바이트다. 값 255는 해당 추첨값이나 IRQ가 없음을 뜻한다. 계상을 완료한 행은 적용값이 1이어야 한다. 모든 IRQ·SVC 및 복귀 주소를 동결 ELF에 대조한다. 실물 지연·간섭 및 제품 보장은 별도 추적한다.

[EVD-021](evidence/evd-021.md)

## 최초 실패 관측 (#79)

CHG-037 / IMP-035, POL-010, 예정 EVD-035. 과거 UART는 READY 뒤 PROPORTIONAL FAIL로 끝났다. ELF 작업 루프 안에서 B,A,B,A 순으로 IRQ가 네 번 발생했다(0x4020127c, 0x40201260, 0x4020127c, 0x40201260). 세 번 복귀했다. 마지막 중단은 epoch 0의 슬롯 3이다. 당시 타이머 값, 레지스터와 판정식 기록이 없어 정확한 원인은 미해결이다.

proportional-diagnostic 설정에서 FAIL 출력 전에 최초 실패의 소스 줄, 0부터 시작하는 epoch와 슬롯, 실측 및 기준 피연산자를 보존한다. 줄 번호와 비교 연산은 해당 ELF 리비전의 proportional.rs를 따른다. 불리언 또는 정책 결과 누락은 0/1로 표시한다. 기존 심볼 기반 QMP 관측기로 통과 및 거부된 실행 모두에서 독립 실패 레코드를 덤프한다. proportional-fixture-miss를 명시하면 첫 추첨 타이머만 두 틱으로 늘리고 정상 합격 기준 한 틱을 유지한다. UART 거부, epoch 0 슬롯 0의 최초 기록 및 실측 행 일치를 확인한다. 더 이른 자연 실패는 그대로 관측한다. 일반 실행과 계측 실행의 증거를 따로 보관한다.

리눅스에서는 kernel/에서 선택한 기능을 빌드하고 매 실행마다 새 수집 디렉터리를 지정한다.

```sh
PROPORTIONAL_CAPTURE_DIR="$capture" python3 tools/qemu_boot.py --profile proportional --runs 1 --kernel "$elf" --qemu tools/proportional_capture.py
```

trace.bin은 리틀엔디언 u64 다섯 개(소스 줄, epoch, 슬롯, 실측, 기준)이다. 줄이 0이면 확정된 실패 기록이 없다. 첫 epoch 전에는 두 좌표 모두 u64::MAX이다. 래퍼는 QEMU 기동 전에 SIGTERM 처리기를 설치하고 UART를 실시간 전달한다. UART 거부 시에도 종료 처리 중 QMP 덤프를 실행한다. launch.json의 심볼 proportional_failure와 크기 40을 확인하고 exit.json, uart.bin, audit.bin을 독립 대조한다. 배열은 순서상 최초 불일치 원소 쌍을 기록한다. 각 ELF와 대응 소스 트리를 함께 보존한다. 과거 ELF SHA-256은 3e771ec6fb331510ffe5f719c3e1eb4840a0c7105af8fd4ce49bed89f132a070이다.
