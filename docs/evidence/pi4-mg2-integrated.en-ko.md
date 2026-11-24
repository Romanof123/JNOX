# Integrated Pi 4 migration image verification / 통합 Pi 4 이주 이미지 검증

The global, partitioned and clustered MG2 builds at `648624b22e275b7c01b57ee915cbefc8f6fdd7d7` matched `559449c673ac55a0a2bf34f3e8ae680d99e72f09` in entry, all PT_LOAD headers and bytes, linked input loop instructions, ERET sites and selected stack symbols. All three full ELF hashes changed. The integrated decoder replayed each original first session. MG2 retained **0/6**: M finished before the migration IRQ in every case. Historical MG1 retained **1/6**, partitioned position 1. No new boot was needed for these identical load images.

전역·분할·클러스터 MG2 이미지는 기준 소스와 진입점, 적재 헤더 및 바이트, 입력 루프 명령, ERET 주소, 스택 심볼이 일치했다. 전체 ELF 해시는 세 경우 모두 달랐다. 통합 디코더로 기존 최초 세션을 각각 재생한 결과 MG2는 여섯 번 모두 M이 이주 IRQ 전에 끝나 **0/6**이었다. 과거 MG1에서는 분할 첫 회차만 충족해 **1/6**이었다. 적재 이미지가 일치해 추가 부팅은 0회였다.

Twelve other locked Rust 1.95.0 release builds compared ordinary and engineering Pi RR, v1 FIFO/RR, directed-wake, ordinary PI/ceiling, legacy mutex, three AMC cases and legacy AMC against the verified #159 and #165 references. Entry, load headers and bytes, and selected stack/restore ranges matched in every case. Directed-wake matched the entire #156 ELF. Archived ordinary RR replay passed 2/2. Both engineering captures retained ambiguous B-acquire brackets (0/2). Archived finite AMC replay passed 3/3. These model observations establish neither physical migration nor timing bounds.

상호 배타적인 설정 12개도 별도로 빌드해 #159·#165 기준과 비교했다. 일반·엔지니어링 Pi RR, v1 FIFO/RR, directed-wake, 일반 PI·천장, 기존 뮤텍스, AMC 세 사례와 기존 AMC 모두 진입점·적재 헤더·바이트 및 선택한 스택·복귀 심볼 범위가 일치했다. Directed-wake는 #156 전체 ELF도 일치했다. 기존 일반 RR은 2/2, 엔지니어링 B 획득의 단일 세대 귀속은 0/2, 유한 AMC는 3/3이었다. 실물 이주와 타이밍 경계는 미측정이다.

[Per-image selectors, hashes, instructions and session replays / 설정별 해시·명령·세션 재생](pi4-mg2-integrated.zip) · [MG2 first positions](pi4-migration-mg2-first.en.md) · [MG1 first positions](pi4-migration-first.en.md)
