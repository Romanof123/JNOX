# EVD-090 · Corrected static footprint verdict / 정적 메모리 판정 재검증

At `812cfd80c62b8936bfd53e5ff4ffe185195375a3` the minimal AArch64 virt boot image met both inclusive static limits. Its ordinary fixed-RR counterpart is a named comparison without an assigned ceiling. The independent verifier reproduced both builds' metrics from the original ELF files after confirming that the correction changed only the measurement tool, its controls and documentation. [Original builds and discrepancies](evd-090-virt-static-first.en.md) · [원래 빌드 및 최초 오류](evd-090-virt-static-first.md) · [PR #185](https://github.com/Romanof123/JNOX/pull/185) · [Finding #188](https://github.com/Romanof123/JNOX/issues/188)

| Bytes / 바이트 | Minimal boot / 최소 부팅 | Fixed RR / 일반 라운드로빈 | RR − boot |
| --- | ---: | ---: | ---: |
| Code and constants, complete RX | 275 | 14,728 | +14,453 |
| Static RAM from RX start to RW end | 20,496 | 115,056 | +94,560 |

All 30 role and boundary checks matched independent readelf calculations. The corrected tool rejected both misaligned BSS-boundary cases and accepted properly linked retained read-only constants. Nine focused and general host tests passed. Earlier outcome labels were visible in the historical manifest. The verifier froze numerical expectations before running the corrected checker or opening the original numerical output. The ZIP contains both language reports, original ELFs, linked controls, program and section listings, command outputs, and 369 checked hashes. SHA-256 `77ad4ec4c71afb77fdc3c62d864a4c8f7b58c920a36fae7496333f697357bfc0`. [Reproducible correction evidence / 수정 판정 원본](evd-090-virt-static-corrected.zip)

최소 부팅 이미지의 코드·상수 275바이트와 정적 RAM 20,496바이트는 각각 256 KiB 및 2 MiB 이내다. 같은 빌드 조건에서 일반 fixed-RR 구성은 코드·상수 14,728바이트와 정적 RAM 115,056바이트였다. 검증팀은 원래 ELF와 별도 판정기로 두 입력 위치 및 경계 사례 30건을 대조했고, 호스트 시험 9건이 통과했다. 기존 매니페스트에 최초 판정 표시는 있었으며, 기존 수치와 수정 도구의 출력은 독립적인 예상값을 고정한 후에 읽었다. 전체 VER-009에서 실제 스택·힙 최대 사용량, 보드 기동 및 지연은 후속 실측 대상이다.
## Blinded check / 사전 판정과 분리한 대조

A separate verifier received ten anonymous ELF files and the acceptance rules. Before accessing the candidate checker or earlier reports, the verifier froze its byte-derived classifications with SHA-256 `efe9cbb1b9c3b0ba8c5a0964ac37a81a3a10326cb594440c26869e5cd03f3f33`. All twenty subsequent input-role outcomes matched. All ten metrics and signed deltas agreed on the sixteen scored reports. As minimal inputs five fit and three exceeded the ceilings; two had invalid layouts. As comparators all eight valid layouts were accepted. Archive SHA-256 `3dcae3f550ee6e2b5c4fc71e3fc2948d01413d944a6e1b843b79e58a5a3af71b`. [Blind inputs, freeze and observed checks / 익명 입력 해시·사전 판정·관측](evd-090-virt-static-blind.zip)

별도의 검증팀은 이전 출력과 도구를 보기 전에 익명 ELF 10개의 허용 여부와 크기를 먼저 확정했다. 각 파일을 최소 및 비교 입력으로 각각 넣은 20건의 종료 판정이 사전 판정과 일치했고, 수치가 있는 16건에서 10개 지표 및 차이의 부호가 일치했다. 원래 ELF는 최초 보관물에, 익명 입력 해시와 판정 원본은 위 파일에 보존했다.
