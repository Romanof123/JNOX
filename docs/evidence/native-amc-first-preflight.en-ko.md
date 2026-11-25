# J-NOX #348 개발 통합 첫 사전 점검 / First developer integration preflight

**판정 / Verdict** — 네 C 작업의 첫 컴파일이 모두 실패했다. Each first C build exited 1 before linking. Both deployment packages are blocked at the application build.

| 항목 / Item | 기록 / Observation |
| --- | --- |
| 소스 / Source | Public feature/native-amc-applications at `da689d6356c9b97e585d1c3c0d0e5ae791594dc3`, fresh shallow clone, WSL Ubuntu, `/dev/sdd` ext4 |
| 도구 / Tools | Ubuntu Clang 14.0.0-1ubuntu1.1, LLD 14.0.0, Rust/Cargo 1.95.0, Python 3.10.12, GNU nm 2.38 |
| 실행 / Commands | Per README, `sh sdk/native-aarch64/build.sh examples/native-amc/slotN.c output/native-amc/slotN.elf` for N=0,1,2,3, serially |
| 최초 결과 / First results | Slots 0, 1, 2, 3 each exited 1 with empty stdout. Each stderr reports `examples/native-amc/ring.h:82:21: error: use of undeclared identifier 'CLOCK_PROCESS_CPUTIME_ID'` |
| 진단 / Diagnosis | 공개 C 헤더에 프로세스 CPU 시계 상수가 없다. The SDK time header publishes CLOCK_MONOTONIC only. The AMC kernel accepts clock ID 2 for per-generation CPU time. |
| 검증 범위 / Scope | ELF 산출 전 실패하여 within-lo·normal-hi 패키징, 거부 사례와 게스트 실행은 미수행이다. No sample ELF was produced. Packaging, rejection cases and guest runs were unexecuted. |

원본 명령, 종료 코드와 표준 출력·오류는 `raw/slotN.{command,exit,stdout,stderr}`에 있다. The toolchain transcript, runner and SHA-256 of the relevant frozen source inputs accompany the four raw results under `raw/`. `SHA256SUMS` covers this report and the raw files.
