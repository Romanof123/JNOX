# Cross-profile integration / 교차 프로필 통합 검증

PASS on merged tree b77bc2f142cb8b50933989d683c829fed05fb2b4 (commit 3a58446f303b0e8a3603f2991082899d5c8a4e66). Four first cold boots, four fresh locked release builds, one build job and one Cortex-A72 QEMU guest at a time. All four serial streams reached their application endpoints before the 4-second host timeout. Two shell parsing failures preceded the first build and guest launch.

| First position | Guest result |
|---|---|
| 1. Telemetry, SJF | Unknown 4 and length 3 rejected, jobs 0–2 admitted, repeated 0 full. Job order 2,1,0. Error 4, threshold count 3, summary count/min/max/sum 3/0/65535/65542. Positive durations 6458,1059,23 counter ticks respectively. |
| 2. Recurring groups | Block and wake, three ACKs with contributions 33152,33408,33664 and cumulative 33152,66560,100224, then peer, resume and return. |
| 3. Ordinary batch, SJF | Estimate, full and stale rejection, overrun retained, continuation and APP FINISHED. |
| 4. Ordinary groups | Block, wake, peer, resume and return. |
| 5. Incompatible build | Groups with recurring app and telemetry workload rejected at build.rs:68 with cargo exit 101. |

한국어 결과. 병합 트리에서 SJF 원격측정 작업 세 건과 반복 그룹 누계 100224를 최초 부팅으로 확인했다. 기존 배치와 그룹 앱도 종료 지점에 도달했고 그룹에 원격측정 선택자를 결합한 빌드는 거절됐다. 이 통합 판정은 일반 모드의 가상 단일 CPU 앱 동작을 다룬다.

[ELFs, UARTs, exception streams and build records / 실행 파일 및 원본 로그](cross-profile-integration.zip) · SHA-256 b0317441e34be3fc727090cd9ee10109e5cbd6325b222e95a6cf76fb252e1f12
