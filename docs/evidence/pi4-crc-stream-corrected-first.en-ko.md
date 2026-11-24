# Pi 4 finite CRC stream · 최초 6회 독립 검증

**6/6 qualified on stock Pi 4 QEMU. 최초 여섯 건 모두 모델 기준 통과.**

Source / 소스 · b14b044f1e548f13ddf48bd5bfa5f91001cabcca. Plan / 계획 · 3a76d3e83ac5576a89ed0770cb3747e59cc26cc1. Clean checkout, one build or guest at a time. All three images and byte-loop disassemblies frozen before global 1. 별도 클린 빌드에서 세 이미지를 실행 전에 고정하고 정해진 순서대로 한 번씩 실행했다.

Stock SHA-256 · 3ae2c079d4491362fece2abb641f0ff17d88e9008c9fcd13600f249b9bd1c862. Ordinary model only, no faults or hardware. 일반 모델 실행이며 결함 주입·실기 검증은 수행하지 않았다.

### Build selectors / 빌드 선택

Three CRC and three legacy sum applications built and froze. Wrong board, wrong application, extra benchmark and partial virt cooperative combination each failed in Cargo build.rs (101). Selector-free virt --all-features built cooperative (0). 세 스트림 앱과 기존 합산 앱 세 건 모두 빌드·고정. 잘못된 선택 네 건은 거부됐으며 virt 전체 기능 빌드는 협력형으로 성공했다.

### First positions / 최초 실행

PC is the matched external IRQ/ERET address. SP and X21 remained identical at resumed dispatch. Each before/after byte count is a retained checkpoint wholly owned by its lease. 표의 전후 진행 지점은 각각 단일 리스 소유이며, IRQ와 ERET의 PC, 복귀 SP·X21 및 독립 접두 CRC를 확인했다.

|Case / 배치|#|IRQ/ERET PC|SP|X21|CPU; lease / 리스|Bytes before → after (prefix CRC)|
|---|---:|---|---|---|---|---|
|global|1|0x8220c|0x910d0|0x910e0|0→1; 1→4|2048 (8dd0b4af) → 135168 (4da7be6f)|
|global|2|0x8220c|0x910d0|0x910e0|0→1; 1→4|2048 (8dd0b4af) → 148480 (bc8fb10a)|
|partitioned|1|0x82210|0x910d0|0x910e0|0→0; 1→5|2048 (8dd0b4af) → 136192 (98870dae)|
|partitioned|2|0x82210|0x910d0|0x910e0|0→0; 1→5|2048 (8dd0b4af) → 124928 (b0c645f8)|
|clustered|1|0x822bc|0x910d0|0x910e0|0→0; 1→5|2048 (8dd0b4af) → 116736 (f368aa39)|
|clustered|2|0x822bc|0x910d0|0x910e0|0→0; 1→5|2048 (8dd0b4af) → 167936 (41445086)|

Loop ranges (exclusive end) / 링크 루프 [시작, 끝) · global [0x8220c,0x8224c), partitioned [0x82210,0x82250), clustered [0x822bc,0x822fc). X21 anchors byte count at +0 and CRC at +8. X21이 가리키는 로컬 상태의 바이트 수와 CRC를 실제 루프가 읽고 갱신했다.

All six completed 4,096 blocks and 4,194,304 bytes, CRC 0x78d22ad1, ordered digest 8765611335505774534. Independent zlib regenerated every expected prefix. UART carries sparse owned checkpoints and an ordered summary of all blocks. Terminal peer products 528, 136 and 392 also matched. 여섯 건 모두 최종 CRC와 4096개 체크포인트 순서 다이제스트, 피어 결과가 일치했다. UART 개별 행은 일부 블록을 보존한다.

Original c2ef996 build failures precede these positions. The earlier six sum-model positions remain 0/6 for the inner-loop criterion. 기존 빌드 실패와 합산 앱의 0/6 판정은 별도 기록이다.

[Six stock model captures, builds and replay / 모델 최초 6회와 빌드 및 재생 자료](pi4-crc-stream-corrected-first.zip) · SHA-256 555ea3a80670e3e3c1c8ff2c6b5480715f5e1cb432780b7150877e59b3a172da.
