# Independent catalog review / 독립 근거 목록 검토

ACCEPT #206 at ea95956a580b5126d0b808d30426a850ae5dceb1 for the #205 catalog repair under #6. The scheduling audit on 2a24bf993a4f326437ad93b1d84bc780410c1417 reproduced four findings. The exact candidate produced zero. The requirement graph exited 0 on both. Both checkers and their test files have identical Git blobs. Both canonical records in English and Korean linked to the intended sources, reports, indices and archives. All six archive SHA-256 values matched historical Git objects and published pins. The raw receipt lists the full digest, publishing commit and all five subject/tool source hashes.

For EVD-058, replay of nine retained sessions reproduced continuous 1/3, finite 3/3 and integrated 3/3 on one emulated Cortex-A72. The two continuous HI cases missed fresh LO at b6 by 10,507 and 9,225 ticks per the original record. Replayed finite and integrated HI cases each produced eight samples summing to 36. Product carry-in, physical timing and M5 response remain open.

For EVD-090 the static subjects were the original minimal and fixed-RR ELFs built at 34454f9. The corrected checker at 812cfd8 measured RX/RAM of 275/20,496 and 14,728/115,056 bytes. Only the minimal image had inclusive 256 KiB/2 MiB limits. Re-execution reproduced all 30 corrected input-role checks and 20 digest-mapped blind outcomes. The first checker had accepted a misaligned BSS end and rejected valid retained read-only data. Runtime stack and heap peaks, startup and physical latency remain open.

Four real per-locale document removals triggered findings. Eight identifier substitutions using an ASCII prefix or underscore suffix triggered provenance mismatches across both languages. Four middle-dot delimiter substitutions passed. Removing the tested-in edge triggered five findings. Removing the policy-evidence edge escaped the scheduling checker on this candidate. An independent exact-set check verified all 13 required typed edges and detected each simulated single omission. This limits reliance on the general checker alone. No guests, builds or remote writes occurred.

## 한국어 판정

#6 독립 검토에서 #205의 목록 복구에 한해 위 #206 커밋을 승인한다. 기준 커밋에서 스케줄링 검사 오류 4건, 후보에서 0건을 재현했다. 두 요구사항 그래프 검사는 종료 코드 0이었다. 두 검사기와 시험 파일의 Git 객체가 동일했다. 두 기록의 영어·한국어 정본 링크를 모두 확인했다. 원본 ZIP 6개의 해시를 과거 Git 객체 및 공개 해시와 대조했다.

EVD-058의 단일 에뮬레이션 Cortex-A72 세션 9개를 재판독했다. 연속 작업은 1/3, 유한 및 통합 작업은 각각 3/3이었다. 연속 HI 두 건은 원본 기록 기준 b6보다 각각 10,507틱과 9,225틱 늦었다. 유한·통합 HI에서 각각 표본 8개와 합 36을 재현했다. 제품 이월 작업량·실기 시간·M5 시스템 대응은 미결이다.

EVD-090의 측정 대상은 34454f9에서 만든 기존 최소·fixed-RR ELF이며 812cfd8은 수정 측정 도구 소스다. RX/정적 RAM은 각각 275/20,496 및 14,728/115,056바이트였다. 256 KiB/2 MiB 상한은 최소 이미지에만 적용된다. 수정 판정 30건과 익명 입력 역할 판정 20건을 재현했다. 최초 도구는 BSS 끝 정렬 오류를 허용했고 유효한 읽기 전용 섹션을 거부했다. 실행 중 스택·힙 최대치, 기동 및 실기 지연은 미결이다.

언어별 파일 제거 4건과 식별자 경계 훼손 8건은 검출됐고 가운데점 구분 기호 4건은 통과했다. tested-in 연결 제거로 오류 5건이 발생했다. 정책 근거 연결만 제거하면 기존 검사기가 이를 놓쳤다. 독립 집합 대조에서는 필수 연결 13개 및 각 단일 누락을 구분했다. 게스트 실행·빌드·원격 쓰기는 수행하지 않았다.

[Independent control bundle / 독립 대조 재생 자료](scheduling-catalog-repair-verified.zip) · SHA-256 abd90c5cad38514dc2f86ec64b24b3f982de0e3fd1850ef6f8b1259f59b10c88.
