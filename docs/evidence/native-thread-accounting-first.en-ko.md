# First acquisition / 최초 측정

Plan bb9df529fbf009483b26abf47146f5ef6820ab73. Clean detached checkout ea84f3f60b04d8c95748b425d6253c8e09fb3a4b. WSL2 Linux x86_64, UID 1000, CPU 0, SCHED_OTHER, nice 0, measured TID 807 and companion TID 808. Both clock resolutions were 1 ns. The first and only fixture invocation completed inside the five-second watchdog. Capture and replay each exited 0. A separate Windows Python implementation matched all 35 raw intervals and all integer gates. The companion acknowledged readiness before the trial, counted 47,782 batches strictly inside the busy loop and received the stop signal after the end boundary. All lower/upper values are in first/result.json, with originals in first/stdout.txt and first/stderr.txt.

일반 계정으로 첫 측정을 한 번 수행했다. 시계와 실행큐 대조가 모두 통과했다. 빈 쌍 32개의 최대 벽시계 상한은 39,716 ns, 최대 상하한 차이는 39,337 ns였다. 차단 시 자발 전환 1회와 실행큐 대기 3,677 ns를 관측했다. 경쟁 시 비자발 전환 169회와 대기 45,775,626 ns를 관측했다. 두 작업의 CPU 하한은 각각 40,001,797 ns, 40,002,533 ns였다. 전체 구간별 벽시계·CPU·차이의 상하한, 스케줄러 카운터와 상대 진행량은 first/result.json에 보존했다.

Blocked time outside accounted CPU was 79,976,580–80,205,700 ns. Under contention it was 45,655,275–45,896,278 ns. The thresholds qualify coarse host accounting for this acquisition. Historical guest intervals and attribution remain open. 최초 호스트 측정의 집계값만 검증했으며 과거 게스트 지연 원인은 미결이다.

[First measurement and independent replay / 최초 측정과 독립 재생](native-thread-accounting-first.zip) · SHA-256 13e3fa12ee055ed98acb3fba0aa89c5450003756019c2cff9e9dd8020c4f0504.
