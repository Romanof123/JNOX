# #307 independent first positions / 독립 최초 실행

Source af022d69bf2ad5a62798e163ea72d2daa07be0a5. Plan 7c8925603c492da34f04959f38097cc291dd4d67106d66649f7247e737c581c1. One Cortex-A72, TCG virt GICv2, 128 MiB, 62,500,000 Hz, Q=6,250,000, journal 512, 120 s timeout. Ordered ordinary/traced captures used ext4. Host terminated QEMU after terminal UART and RAM observation. Single-job terminal spelling was corrected in the external observer while the short job remained parked. No guest restart or timeout.

검증자는 공개 커밋에서 C와 Rust ELF를 독립 빌드했다. 원래 LLF 일반·추적, EDF 일반·추적 순서와 최초 결과를 보존했다. 원래 장기 작업의 마감 초과와 별도 배포의 마감 준수를 각각 판정했다.



| Position / 실행 | S | exit counter / deadline (signed margin = deadline − exit) | raw service A,B |
|---|---:|---|---|
| llf-plain | 6749383 | 19290552 / 44249383 (+24958831), 123607607 / 50499383 (-73108224) | 18210, 116648085 |
| llf-traced | 6776117 | 19319600 / 44276117 (+24956517), 120876317 / 50526117 (-70350200) | 20425, 113869022 |
| edf-plain | 6727722 | 6854661 / 44227722 (+37373061), 119016292 / 50477722 (-68538570) | 23200, 112071009 |
| edf-traced | 6716118 | 6822007 / 44216118 (+37394111), 118107418 / 50466118 (-67641300) | 20307, 111201210 |
| short-plain | 6482304 | 6593591 / 31482304 (+24888713) | 22028 |
| negative-plain | 6555942 | 118940157 / 12805942 (-106134215) | 112255537 |
| practical-plain | 6734181 | 19273931 / 44234181 (+24960250), 21480166 / 50484181 (+29004015) | 18569, 14589500 |
| practical-traced | 6635419 | 19179882 / 44135419 (+24955537), 21607079 / 50385419 (+28778340) | 18616, 14793683 |



Original LLF selected B at k0, retained B on 4/4 at k1, and selected A at 3/4 at k2. Both ready at all three samples, no intervening syscall or release. Model finishes A3, B4. EDF entered A first on both originals. SHA-256 A 086de07c44bbd68540b9fb7149c3f32b5a360e1cc8a4514eb6500413e8bc873e, B e077a9b23513fda4d3f371dc21a9e5d6f36aabc98e31d9720e96a4ea83d3468c. Symbols/disassembly show volatile source-byte load and per-bin increment in B. QEMU traced EL0 IRQ saved 0x400040, kernel recorded SP 0x1010fa0. Initial return entered A at 0x400000; A exit SVC returned to B at 0x400040 on the saved SP. First ordinary run retained terminal private RAM only.

Traced B completed-block range with 2 published journal rows: 7437–14260.

Traced B completed-block range with 4 published journal rows: 14971–21771.

원래 두 정책 모두 모든 실제 제품을 확인했다. 호스트가 QEMU의 128 MiB 게스트 RAM에서 각 슬롯의 페이지 테이블을 순회한 후 사설 페이지를 읽었다. A는 16블록 합 8,355,840, B는 131,072블록 및 16개 빈 각각 33,554,432, 종료값은 각각 17과 29였다. 별도 실용 배포에서 같은 A와 기본 weighted Rust 앱은 일반 및 추적 모두 마감 이내 완료했다. Rust 앱은 67,108,864개 샘플을 집계했고 아홉 빈 전체와 종료값 197을 대조했다. 두 배포는 같은 커널 소스와 스케줄링 정책으로 각자 내장한 ELF가 다른 커널 이미지를 사용했다.

Long solo B observed −1 at k1 and continued to the complete product. Short A finished within its deadline. Rejected (0,1,2) printed REFUSED 8 and FAULT 14 before EL0 entry. CLI and raw Cargo missing/extra tuple controls failed. Host suite passed 163 tests. Inherited historical linked EDF/LLF and external return witnesses retain their original scope. This run independently exercised native EDF in the matching pair. No board or integrated-load timing qualification.

단독 장기 작업의 음수 여유시간과 실제 마감 초과 후 정상 완료를 확인했다. 과수요 패키지는 사용자 실행 전에 거부됐다. 외부 예외 추적은 실제 경로와 복귀 PC를 보여 주며, 커널 기록은 실행 프레임·SP·카운터를 제공한다. 실제 보드 또는 통합 부하의 시간 보장은 검증 범위 밖이다.

Physical endpoint host controls classified equal counter as on time and +1 as late. Original plain LLF lacked pre-switch external progress samples. Traced LLF supplied both sides. The practical plain run also retained terminal RAM only. QEMU IRQ trace identifies IRQ exception class, with PPI30 attribution from the configured guest timer/GIC path.

실용 배포의 9개 빈은 순서대로 262144, 2097152, 7340032, 14680064, 18350080, 14680064, 7340032, 2097152, 262144였다. 최초 일반 LLF의 선점 전 외부 진행량은 미관측이며 최초 추적 실행에서 선점 전후 진행량을 확인했다.
