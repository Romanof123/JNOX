
## Integration with finite AMC / 유한 AMC 통합

Clean locked, single-job Rust 1.95.0 builds on `02d539832be9e5318fc66abd0f2afa04566a8731` preserved all twelve selected entry points, PT_LOAD headers and bytes, and linked stack/restore symbol ranges. The ordinary RR, engineering v2 RR, v1 FIFO/RR, directed-wake, ordinary PI/ceiling and legacy mutex references were built at `42a1ffe678a015913fda9c8656a30e204927dafd`. The three finite AMC references and legacy AMC came from verified `6a336318e265785b5c0d8df3b5a8649632520919`. Directed-wake and all four AMC ELFs matched in full. Replaying the archived UART with the candidate RR images again yielded ordinary 2/2, engineering single-generation brackets 0/2. Both engineering records remain structurally valid. The candidate AMC decoder reproduced 3/3 on the retained integrated finite cohort. New boots: zero.

통합 소스의 12개 설정을 각각 새로 빌드한 결과 진입점·적재 헤더·적재 바이트 및 링크된 스택·복귀 심볼 범위가 기준과 일치했다. Directed-wake와 AMC 네 설정은 전체 ELF도 일치했다. 기존 UART와 새 RR 이미지를 함께 재생한 결과 일반 RR은 2/2, 엔지니어링 B 획득의 단일 세대 귀속은 0/2였다. 엔지니어링 두 기록의 구조는 유효했다. 통합 AMC 유한 작업의 보존된 세 사례는 모두 재검증됐다. 추가 부팅은 0회였다. [Selectors, binary hashes, symbols and replay / 설정·해시·심볼·재생 결과](pi4-rr-after-amc.zip)
