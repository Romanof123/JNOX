# POL-008 ordinary reservation timing / 일반 예약 타이밍
Head `dec753ac75719f1e29ff9622bce74ab11f7d06c2`. Three of eight archived first boots completed the prescribed horizon. The 8/8 timing criterion failed. 최초 실행 8건 중 3건이 지정된 지평선을 완주하여 8/8 기준을 충족하지 못했다.

|Position / 순서|Workload / 작업|Trace / 추적|Terminal slot / 목표|Returned worker grants / 작업자 복귀|First worker marker after epoch (ticks) / 최초 표지|Failure (ticks over 62,500) / 초과량|
|---:|---|---|---:|---:|---:|---|
|1|aligned-wake|off|0/34|0|—|prepublication / 게시 전 +1,585|
|2|cbs-wake|on|34/34|17|58,086|—|
|3|aligned-saturated|on|0/10|0|—|prepublication / 게시 전 +1,495|
|4|cbs-saturated|off|10/10|8|54,263|—|
|5|aligned-wake|on|1/34|2|55,768|slot 1 marker / 슬롯 1 표지 +5,421|
|6|cbs-wake|off|0/34|1|63,341|slot 0 marker / 슬롯 0 표지 +841|
|7|aligned-saturated|off|10/10|8|51,304|—|
|8|cbs-saturated|on|0/10|1|62,643|slot 0 marker / 슬롯 0 표지 +143|

All 37 worker returns matched measured service records by slot and owner. Per-grant service used the sampled close minus the pre-return timestamp. Cumulative charges matched the workload ledger. The three recorded grant-level violations were late worker markers. Positions 1 and 3 stopped before a worker return. All eight archive SHA-256 values and every member matched the supplied manifests. At 62.5 MHz the inclusive opening limit is 62,500 ticks. The archived candidate and parent ELF entry and loaded segment comparisons matched for all four legacy benchmarks and differed for all four ordinary reservation images. See loaded.json for per-segment hashes and metadata.

작업자 복귀 37건을 슬롯·소유자별 실측 서비스 기록과 대조했다. 종료 시각에서 실제 복귀 직전 시각을 빼서 서비스량을 구했고, 누적 과금은 작업 원장과 일치했다. 반환된 승인에서 관찰된 위반 3건은 모두 작업자 표지 지연이었다. 1번과 3번은 작업자 복귀 전에 중단했다. ZIP 8건과 각 내부 파일의 SHA-256이 외부 매니페스트와 일치했다. 62.5 MHz 기준 허용치는 62,500틱 이하이다. 후보·부모 ELF의 진입점과 적재 세그먼트 비교에서 기존 벤치마크 4종은 모두 일치했고 일반 예약 이미지 4종은 각각 차이가 있었다. 이 결과는 보관된 실행에 한정된다.

## Replay / 재현

Run `python3 check.py` with Python 3.10+ and AArch64 GNU nm (`aarch64-linux-gnu-nm`) on PATH. It reads the archived images as data and decodes the original UART records. Python 3.10 이상과 AArch64 GNU nm가 설치된 환경에서 위 명령으로 원본 UART 기록을 재검산할 수 있다.

[Eight normal first captures and offline charge replay / 정상 최초 8회 원자료와 과금 재생](reservation-frame-ordinary-first.zip) · SHA-256 35c674cd0cba77b0d975bc16e256f7f5a22b060bc990aa181893c590d92fc26c.
