# 복권 스케줄링과 실행량 기반 스트라이드

[English](m4-proportional.en.md) · [변경 이슈 #54](https://github.com/Romanof123/JNOX/issues/54)

CHG-023 · POL-010 · REQ-002/005/023 → DES-003/012 → IMP-021 → VER-002/005/023. Cortex-A72 한 개와 GICv2에서 전용 스택을 가진 A·B·C의 가중치를 1·2·4로 둔다. 복권 사례는 초당 100회 기준 물리 타이머 허용을 시드당 32회 수행한다. SplitMix64의 초기 상태는 1과 2다. 활성 티켓 합으로 2^64를 나눈 나머지보다 작은 원시 난수는 버리고, 다음 적합한 값의 나머지로 추첨한다. 세 작업이 준비 상태이면 0은 A, 1~2는 B, 3~6은 C를 선택한다. 구간 밖 추첨은 거부한다. 시드별 선택 횟수와 32 × 가중치 / 7의 차이는 작업마다 최대 8회다. 호스트에서 시드 1~8에 각각 4,096번 추첨하고, 시드별·작업별 비율 오차를 3%포인트까지 허용한다.

스트라이드는 패스가 최소인 준비 작업을 고르고 동률이면 작은 ID가 우선한다. 활성 작업의 최소 패스를 함께 빼서 상대 순서를 보존한다. 초당 25틱 기준 실제 경과한 정수 틱에 720,720 / 가중치를 곱해 계상한다. A는 1틱 후 양보하고 B·C는 2틱을 쓴다. A·B만 준비된 첫 여덟 번은 A,B 네 차례 반복으로 서비스가 4·8·0이어야 한다. C를 기존 작업의 최소 패스에서 합류시키면 이후 A,B,C,C가 두 번 반복돼 2·4·8틱이 추가된다. 합류 전후 패스, 정규화 및 반환 후 사용량을 기록한다.

kernel에서 `cargo build --release --locked --features proportional-bench -j1`로 빌드한다. 저장소 루트에서 `python3 tools/qemu_boot.py --profile proportional --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel`를 실행한다. 첫 관측은 새 PROPORTIONAL_CAPTURE_DIR에 저장하고 `python3 -B tools/proportional_capture.py "$@"`를 호출하는 Linux 래퍼를 --qemu에 준다. 부팅 후 표식 네 줄은 LOTTERY SEED A, LOTTERY SEED B, STRIDE SERVICE JOIN, PROPORTIONAL PASS 순이다.

ELF 심볼 proportional_witness는 7,104바이트다. 앞의 리틀엔디언 u64 다섯 개는 행 수·복권 틱·스트라이드 틱·두 시드이고 다음 u32 여섯 개는 합류 전후 패스다. 80개 행은 각각 u64 여섯 개(ELR·문맥·시작·정지·타이머 비교값·진행량), u32 일곱 개(정규화·선택 시 패스 셋·반환 후 패스 셋), 12바이트(사례·슬롯·소유자·준비 마스크·서비스·인터럽트 ID·추첨값·총 티켓·구간 시작/끝·패딩 둘)다. 255는 해당 추첨값이나 IRQ가 없음을 뜻한다. 모든 IRQ·SVC를 동결 ELF의 실행 루프 및 복귀 주소에 대조한다. 첫 실행 결과를 기록한 후 구현을 검토한다. 실물 지연·간섭 및 제품 보장은 별도 추적한다.
