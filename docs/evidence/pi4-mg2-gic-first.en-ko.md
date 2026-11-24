# Independent stock GIC observation / 기존 GIC 모델 독립 관측

Frozen public criteria d2688cb8141ddd9a83d58698af87383961e3f1cc. Collector 5515dd30aeb3a3424964bca5f91e6bc75226ed04. Three parser controls passed. Both original positions ran on native WSL ext4 in the fixed OFF, ON order. Each complete raw inventory was copied to D:, reopened, hashed and replayed before any further guest launch. Both replays returned 0. The freeze and position manifests identify every input and raw file. The supplemental guest JSON decodes the saved RAM using the candidate's Stage 1 arithmetic and frame checks.

|Measurement|OFF|ON|
|---|---:|---:|
|Host elapsed ns|1136977115|1126939036|
|Terminal quiet ns|1016144391|1017460639|
|Guest terminal error|0x1d|0x1d|
|CPU0/1 committed rows|16/9|16/9|

CPU1 guest row 7 records 0x18001 in both runs. OFF has zero events of all six enabled kinds under test. ON CPU1 TID 811 lines 17–19 records the opening distributor write, CPU0 IRQ1 winning candidate (128/255/256 priority/mask/running), then CPU0 IRQ level 1 decision. Pending-bit storage and application of the modeled line are untraced. There is no invocation ID. CPU0 TID 810 shows no architectural IRQ, acknowledge, IAR 0x401 or EOIR. Both guests have no IRQ trap or EOI row and no reverse send. No receiver chain or local trap ordering can be joined.

Both CPU0 journals have M lease 1 row 14 at index 65536 with sum 2147516416. M's SVC frame ELR 0x8204c matches the CPU0 exception history, count 4096 and fold 18080812676020141577. CPU1 P1 lease 2 completes index 16, sum 136 (rows 4/5). CPU0/1 both stop at 0x811cc, CPU2/3 at 0x310. Both journals are unsealed with zero lost rows. Both UARTs report M finished before IRQ, then failure 0x1d. Neither shows the required interrupted inner-loop PC and strict restore on another CPU. The historical strict cohort remains 0/6. The model has no common causal record for the CPU0 checkpoint and CPU1 SGIR. The single pair yields neither repeatable tracing overhead nor physical interrupt latency.

고정된 OFF→ON 첫 시도 모두 RAM·스레드·UART 수집 및 재생에 성공했다. 두 실행 모두 CPU1 게스트 7번 행에서 0x18001을 보냈다. ON의 CPU1 스레드 811번 17~19행은 분배기 쓰기, CPU0 IRQ1 후보 선택, 출력 레벨 1 결정을 차례로 보여 준다. pending 저장과 실제 출력 적용은 직접 기록되지 않았다. CPU0의 IRQ 진입·ack·IAR·EOIR, 게스트 트랩·EOI, 역방향 송신은 관측되지 않았다. CPU0의 M은 두 번 모두 리스 1, 14번 행에서 인덱스 65536·합계 2147516416에 도달했다. 이어 산술 검증을 통과한 종료 SVC가 관측되었고 UART는 오류 0x1d로 끝났다. 두 실행 모두 다른 CPU에서 내부 루프를 복원했다는 엄격한 기준을 충족하지 못했다. 종전 판정은 0/6이다. 서로 다른 CPU의 체크포인트와 송신 순서 및 지연은 결정할 수 없다.

[Two original model traces and independent replay / 두 모델 실행 원본과 독립 재생](pi4-mg2-gic-first.zip) · SHA-256 bc1e9c33329d76283ee6fef5f20154e06f7074e65e59379ceb690622505fb495.
