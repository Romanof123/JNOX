# Independent release review · 독립 릴리스 검증

PASS for documentation integration at head a733a1f891fb5e6b9be7b24e8cd9766204a84efd (standalone tree 3c4a00710c7eda7a8ae0f5bf391bbe5c45101b19). Main a658576e7697051b0bf2ecb1d9d81e4c3eeb78c5 and merge base 2c3b813e3425391d36f83a0bb788e0025955e3ec yield integration tree 41f52b05c17dce2beca82d455e554808b1875fb1 by both merge-tree and independent index merge. Four changed files, two README summaries, five policy rows in each language. Other thirteen rows and all eighteen open release-gate cells match main. Graph and scheduling evidence return zero, no findings, 33 focused offline tests pass. Zero new guests.

양 언어 README와 다섯 정책 행의 원자료를 확인했다. 나머지 13행과 18개 미결 릴리스 조건은 원문과 일치한다. 문서 통합 통과. 신규 게스트 실행은 0회다.

|Policy|Evidence checked / 검증 결과|
|---|---|
|004|Guard restore preceding final check in linked code. Zero directed missed-index observations in 16 first boots. Healthy 0/2, RM phase envelope and DM rearm spacing. 링크 명령 순서만 확인, 지시 경로 실행과 정상 판정 미충족.|
|006|Original 3/4, P/default generation 2 at 73,392 > 62,500. Separate 40 frame boots, P/default 5/5 and Q/default 4/5 over budget for each revision, 20 deterministic passes, five separate compatibility boots. First wrapper 20 pairs lacked symbols. Explicit-build pairs had P plain/phase misses 3/5 vs 5/5, Q 1/5 vs 4/5. Corrected CLI pairs separately had P 3/5 vs 5/5, Q 1/5 vs 1/5. Both measured sets had zero deterministic misses. 계측 효과는 관측됐으며 올바른 공개 패키징에서도 기본 시계 일반 이미지의 최초 전환 초과는 남았다.|
|011|35 host intervals. Single plain/observed share pair, 152 callbacks, 38 grants, 12 positive IRQ outside-CPU lower bounds. Both passes reproduced from archived RAM by offline checker. 과거 지연 원인은 귀속하지 않았다.|
|013|Original sum 0/6, each M finished before IRQ. Three legacy freeze successes and three CRC compile rejections, zero stream guests at preflight. Two distinct CRC runs of six each, all matched full 4 MiB CRC 0x78d22ad1. In each cohort two global resumes on CPU1 and four pinned resumes on CPU0. Integrated legacy .text equal on all three images, three differing loaded bytes per image. 기존 합산 0/6과 기능 브랜치 6/6 및 새 메인 통합 6/6을 구분했다.|
|018|First plain and captured ordinary and legacy boots all returned zero. Nine raw records include three reserved slots. Consumer 62 and reserved monitor 9900. 예약 슬롯 세 번은 확장 평가를 건너뛰었다.|

All newly introduced Markdown references resolve to Git objects. final.txt retains commit IDs, evidence-document and original-archive SHA-256; manifests retain hashes of each archived member. claims.txt contains independent per-case computations. The compressed delivery omits upstream archives, binaries and full memory dumps. Local replay requires a JNOX checkout named jnox227-release next to jnox227-release-proof, Git, Python and pyelftools. From their common parent run audit.py with the final head and redirect stdout to final.txt, then inspect_tables.py and check_claims.py. Tests ran from tools with python -m unittest test_requirement_graph test_scheduling_evidence test_pi4_stream_v1 test_ordinary_partition_account test_partition_selectors test_native_el0_uart test_vruntime_observer.

[Independent source, evidence and integration checks / 독립 문서·원본·통합 검사](native-smp-release-dispositions-review.zip) · SHA-256 96231d1ede0678f0fc329175b324045705687bff610f1cfac20b8ca78fab8b90.
