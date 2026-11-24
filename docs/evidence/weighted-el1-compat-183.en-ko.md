# EL1 weighted scheduler compatibility / EL1 가중 스케줄러 호환성

At `af952e34cea8fbe0aef5c7b4cb1f945a2884fff5`, all five EL1 selector builds reproduced the full ELF bytes from `56d6e0b7cb7183fdc1b9b06275050ce64254e6f6`. The comparison covered weighted-share and weighted-wake, each plain and with JNOX_WEIGHTED_TRACE=1, plus the vruntime-bench feature. Each pair used the same checkout and target paths, Windows Rust 1.95.0, AArch64 Cortex-A72, locked release, one build job and a fresh target. Entry and all PT_LOAD headers and payloads also matched.

The original EVD-049 archive and its member manifest were verified before replay. Its independent verifier recovered all eight original observations across default and instruction-counted clocks. The candidate decoder accepted all eight UART transcripts (38 closures per share run, 21 per wake run). The benchmark observer also accepted the original EVD-022 UART, which records 21 choices. Those historical ELF digests differ from the two revisions compared here. This verifies compatibility of the artifacts and readers in their respective scopes. Additional guest launches: zero.

두 리비전에서 EL1 일반 가중 분배·깨우기의 일반/추적 설정 네 가지와 기존 vruntime-bench를 각각 새로 빌드했다. 다섯 쌍 모두 전체 ELF, 진입점 및 적재 헤더·바이트가 일치했다. EVD-049의 원본 묶음과 내부 해시를 확인한 뒤 독립 검산기가 기존 8개 실행을 재현했다. 후보 판독기는 같은 UART 8건과 EVD-022의 기존 벤치 기록을 받아들였다. 과거 기록의 ELF 해시는 이번 비교 대상과 다르며, 추가 게스트 실행은 0회다.

[Per-selector commands, hashes, load images and capture identities / 설정·명령·해시·적재 영역 및 원본 식별자](weighted-el1-compat-183.json) · [EVD-049](evd-049.en.md) · [EVD-022](evd-022.en.md)
