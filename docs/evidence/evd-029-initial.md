# EVD-029 · CBS 기상 프로필 최초 호출

[English](evd-029-initial.en.md) · [승인 기준](../m2-cbs-wake.md) · [#65](https://github.com/Romanof123/JNOX/issues/65)

관측 결과는 **BLOCKED_BEFORE_BOOT**였다. 호출 과정에서 `/mnt/d/codex/target-cbs-wake` 경로를 찾지 못해 게스트 부팅은 0회였고 캡처 디렉터리도 생성되지 않았다. 검증자는 이 시도에서 재호출하지 않았다. UART, 저널, 예외 추적, EDF 재현 기록이 없어 이 호출로는 스케줄링이나 타이밍을 판정할 수 없다.

ELF SHA-256 `DFBB1D9E04C7219BC1C47FD96AF0F2465E1D924A5C64F5B03AC45757D4731706`. [검증 소스](https://github.com/Romanof123/JNOX/commit/93c64c853c3d1bf3f6286da59d83d7dc8394f7b9) · [최초 판정과 래퍼](evd-029-initial.json)
