# Reservation worker mailbox / 예약 작업자 메일박스 검증

**7/8 first boots completed the full scheduling horizon. / 최초 부팅 8건 중 7건이 전체 스케줄을 완료했다.** Required 8/8. Inclusive dispatch bound remains 62,500 ticks (1 ms at 62.5 MHz). / 통과 기준은 8/8이며 개시 상한은 1 ms 그대로다.

Candidate a667736b639ef9cbe156d9d9978f40a15cadbb91, prospective plan b16ee652fc8e4633ec7563eeaff89d9f4546fe29, technical base a4d2f340020f7e8666d04a113040207805bbfc9a. Fresh locked Windows Rust 1.95 single-job builds. Eight serial Ubuntu ext4 QEMU 6.2 Cortex-A72 TCG guests with original physical timer and timeout. / 윈도우 단일 작업 빌드 후 WSL ext4에서 정해진 순서로 최초 8회 실행했다.

|Pos|App|Trace|Terminal slot/status/reason|Mask|Worker−epoch|Returned grants|First charge|
|---:|---|---|---|---:|---:|---:|---:|
|1|aligned-wake|False|0/2/5|15|—|0|—|
|2|cbs-wake|True|34/0/0|31|54237|17|12288|
|3|aligned-saturated|True|10/0/0|31|56008|8|3026543|
|4|cbs-saturated|False|10/0/0|31|56945|8|3018867|
|5|aligned-wake|True|34/0/0|31|53548|18|11711|
|6|cbs-wake|False|34/0/0|31|49608|17|8926|
|7|aligned-saturated|False|10/0/0|31|54657|8|3018901|
|8|cbs-saturated|True|10/0/0|31|60563|8|3014234|

## First-grant intervals in counter ticks / 최초 승인 구간별 카운터 틱

|Pos|Epoch→IRQ|IRQ→source end|Source end→before|Before→post|Post→armed|Armed→selection|Selection→pre-ERET|Pre-ERET→worker|
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
|1|27848|7857|17039|3714|7273|—|—|—|
|2|17600|6102|13119|3272|6230|2208|2607|3099|
|3|21419|6757|13488|3046|5178|1699|2038|2383|
|4|18502|9155|13407|3064|5514|1944|2605|2754|
|5|19040|5830|13174|2974|5362|2010|2385|2773|
|6|18648|6913|11010|2430|4586|1739|2049|2233|
|7|17817|6244|13522|3046|6385|2210|2666|2767|
|8|27695|6637|13122|2487|4460|1572|2025|2565|

Technical-base position 1 intervals were 25410, 6970, 16956, 3142, 6856, 3018, 2260, 3781 ticks; position 5 intervals were 24540, 6518, 12044, 2580, 4695, 1669, 1973, 2665. These historical samples come from the prospective plan. / 기준 빌드의 1·5번 구간은 사전 계획서의 기록이다. 조건별 1회 관측이므로 배치 변경의 실행 시간 효과를 분리할 수 없다.

84 measured grants joined to their saved return, ordered Rust marker, physical cutoff or SVC trap, progress count/checksum, elapsed charge and cumulative owner/deadline allocation. Full horizons include wake/recovery and retirement checks. Traced positions also compare QEMU exceptions/ERET. Raw terminal operands, per-grant bounds, period ledgers and trace joins are in sealed.json. / 실측 승인 84건의 실제 복귀·Rust 표지·선점 또는 SVC·진행값·기한별 누적 과금을 확인했다. 종료 및 복구 관측은 원본 UART와 sealed.json에 있다.

## Linked layout / 연결 바이너리 배치

|App|Base validator bytes|New validator bytes|Base worker bytes|New worker bytes|Mailbox base address|Data section bytes, base→new|
|---|---:|---:|---:|---:|---:|---|
|aligned-saturated|336|308|288|256|0x40207728|1864→1912|
|aligned-wake|336|308|288|256|0x40207728|1864→1912|
|cbs-saturated|336|308|288|256|0x40207728|1864→1912|
|cbs-wake|336|308|288|256|0x40207728|1864→1912|

In all four candidate images, two 40-byte worker records initialize generation/action/goal/first/progress at offsets 0/8/16/24/32 to MAX/0/MAX/0/null. Source access ordering preserves release goal, relaxed action/reset and release generation, acquire worker generation, release marker, acquire pointer and null check before snapshot. Candidate and base disassembly, symbols and allocated-section SHA-256 are included. / 네 바이너리에서 작업자당 40바이트의 5개 개별 원자값과 초기값, 게시 순서 및 스냅샷 널 검사를 확인했다.

## Legacy controls / 기존 대조군

- periodic-bench matched entry and every PT_LOAD header and byte. / 진입점 및 모든 적재 세그먼트 일치.
- aperiodic-bench matched entry and every PT_LOAD header and byte. / 진입점 및 모든 적재 세그먼트 일치.
- cbs-bench matched entry and every PT_LOAD header and byte. / 진입점 및 모든 적재 세그먼트 일치.
- cbs-wake-bench matched entry and every PT_LOAD header and byte. / 진입점 및 모든 적재 세그먼트 일치.

0/4 changed loads triggered a legacy control boot. / 적재 내용이 바뀐 벤치는 0/4종이었다.
## Position 1 / 최초 1번 실패

Aligned wake/plain epoch 3,179,415 gave an inclusive deadline of 3,241,915. Timer-armed readback at 3,243,146 was already 1,231 ticks late. The guard sampled 3,244,111, 2,196 ticks late, and stopped before selection. The first worker therefore has zero returns, markers and charges. Position 5 ran the same ELF with tracing and completed. The prior 7/8 cohort reached a real but late worker at position 1. / 1번은 타이머 설정 확인 시 이미 1,231틱 초과했고 2,196틱 초과한 검사 시점에 선택 전 중단됐다. 작업자 복귀·표지·과금은 0건이다. 같은 ELF를 쓴 추적 5번은 완주했다. 이전 최초 코호트와 이번 코호트 모두 7/8이었다.

Across all four linked validators, the candidate's five LDAR loads use one 40-byte record stride. In aligned wake these loads are at 0x40202c18, 0x40202c28, 0x40202c34, 0x40202c44, 0x40202c50. The four legacy whole-ELF digests also match. / 정렬 기상 바이너리에서 40바이트 작업자 레코드를 기준으로 5번의 LDAR를 실행한다. 기존 벤치 네 종은 전체 ELF 다이제스트도 동일했다.

[First eight cold boots and independent replay / 최초 8회 실행과 독립 재검증](reservation-worker-mailbox-first.zip) · SHA-256 a069ab408b46fda97d71d8e3ecbdffd465aeea2245790bc02cad91e597417e03.
