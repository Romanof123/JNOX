# Pi 4 main integration · 독립 통합 검증

#229 / #228 · 4d8ea7af9c2a2cc0482025bd56809f5df98b6c66, plan ed0ac5b458aa48f4e37d7379f859a909a62f3dc7, main 2c3b813e3425391d36f83a0bb788e0025955e3ec.

Six of six first stock-model positions qualified. All three legacy executable sections match main byte for byte. Each loaded image differs at three diagnostic source locations. 최초 여섯 모델 실행이 모두 통과했다. 기존 세 이미지의 실행 코드는 동일하며 진단용 소스 위치에서 각 3바이트가 달라졌다.

Stock QEMU SHA-256 3ae2c079d4491362fece2abb641f0ff17d88e9008c9fcd13600f249b9bd1c862. Native scheduling 137/137, host Python 109/109, effective selector matrix passed. Four invalid selector builds rejected; fallback all-features coop and the six intended builds succeeded. 호스트 스케줄링·파이썬 테스트 및 선택기 검증 통과.

|Case|#|IRQ = ERET PC|SP / X21|CPU / lease|Owned prefix bytes (CRC) before → after|Observed M rows|
|---|---:|---|---|---|---|---:|
|global|1|0x82000|0x910d0 / 0x910e0|0→1 / 1→4|2048 (8dd0b4af) → 77824 (c9e9b869)|5|
|global|2|0x82000|0x910d0 / 0x910e0|0→1 / 1→4|2048 (8dd0b4af) → 120832 (e38df19f)|5|
|partitioned|1|0x82004|0x910d0 / 0x910e0|0→0 / 1→5|2048 (8dd0b4af) → 110592 (e1bf88b9)|5|
|partitioned|2|0x82004|0x910d0 / 0x910e0|0→0 / 1→5|2048 (8dd0b4af) → 93184 (3e529239)|5|
|clustered|1|0x820b0|0x910d0 / 0x910e0|0→0 / 1→5|2048 (8dd0b4af) → 73728 (07e0aab9)|5|
|clustered|2|0x820b0|0x910d0 / 0x910e0|0→0 / 1→5|2048 (8dd0b4af) → 119808 (2fa578c5)|5|
All 4,096 expected prefix CRCs regenerated independently using zlib. UART contains sparse per-lease samples and one ordered summary for 4,194,304 bytes, CRC 0x78d22ad1 and digest 8765611335505774534 per position. Peer products 136, 392 and 528. 각 실행의 UART는 일부 접두 행과 전체 순서 요약을 보존한다. 독립 생성한 4096개 예상 접두값은 CSV에 담았다.

## Legacy controls / 기존 경로

One first control launch per base/candidate policy, read by both old and new tools. Wire v2 schema, flags, QEMU hash, timing and ordinal manifests identical except source/artifact hashes. Image offsets 20680, 20736 and 20808 encode Rust panic source line numbers in .rodata, changing 269→270, 487→488 and 333→334 in src/pi4_migration.rs. Full executable, workload, IRQ/return paths and scheduler/context machine instructions identical. 기존 실행 코드와 매니페스트 설정이 일치하며 .rodata의 패닉 소스 줄 번호만 달라졌다. 기본 합산과 스케줄링 정책은 그대로이며 조기 종료에서는 최종 UART 합계 증거를 얻지 못했다.

|Control|Image SHA-256|Old and new reader outcome|

|---|---|---|
|base-global|7082200fe27285b9505bebe50f772f9d41455747d7472db3635c3edaa60358ee|M finished before migration IRQ|
|legacy-global|84115b8a1184a3becea2f26eb62bc8e32d6421bb97049cb8bf14febebf011497|M finished before migration IRQ|
|base-partitioned|ba8cd99566372ae31e5c25debedd7cc9496df79c3bb272bfe57b9f59053739db|M finished before migration IRQ|
|legacy-partitioned|937ac2913c3d5949c3f49562049a40dee7eb9f24af91c20d7a3338d7472c126c|M finished before migration IRQ|
|base-clustered|4fb7fcdad70a76a725fb05ea433d9cdff2b250622037049f262d15c120bcd131|actual continuation matched; qualifying inner-loop interruption unobserved|
|legacy-clustered|739eb30bce9c8832226411b41a483529c3eb97186126ba343db1abddff6ee6ba|M finished before migration IRQ|
Legacy workload target 4,096 checkpoints, sum 2,147,516,416 and digest 18,080,812,676,020,141,577. Only completed control summaries can attest these at runtime. 이번 검증은 결함 주입 없이 stock Pi 4 QEMU에서 수행했다. 실기 결과는 포함하지 않는다.

[Frozen mainline builds, raw runs and independent replay / 본선 빌드와 최초 실행·독립 재생 원본](pi4-crc-mainline-first.zip) · SHA-256 a0ec8c9cfe651c4e0c3b6d74006b52e36eb616882b051701e65ef84f74a904da.
