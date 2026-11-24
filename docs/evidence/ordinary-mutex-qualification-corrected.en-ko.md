# Corrected #242 independent replay / #242 수정판 독립 재생

PASS for the prospective #241 offline contract (f4140f2, correction 1bb4f20). #241 사전 판정 계약과 수정 계약에 대해 오프라인 재생을 통과했다.

Executed combined Git tree 1688139337286375222cae25a307248cf9739a59, local commit 8a43530a22574f7871ac9fbce2a7f8ebea7b04c9. Parents, in order, a3e68327e31e8a70befd766b3d374bf86521d46b (main) and ee48764ba58634c4d8f3c2dd0d8d0b037dea57e7 (PR). The detached worktree was clean after replay. 두 공개 부모의 실제 병합 트리에서 검사했다.

| Position / 위치 | Feature IRQ chunks / 기능 | Integration IRQ chunks / 통합 |
|---|---|---|
| PI/plain | 42 qualified | 57 qualified |
| PI/traced | 76 qualified | 0 unqualified, reason 16 |
| ceiling/plain | 80 qualified | 18 qualified |
| ceiling/traced | 29 qualified | 0 unqualified, reason 16 |
| Score and exit / 점수와 종료 | 4/4, 0 | 2/4, 1 |

Read both original ZIP blobs from public 317d7365. Frozen hashes are in the transcript. All four original traced exception entry and return address sequences matched the corresponding UARTs. Six qualified launches ended with HIGH [1,65536,557056]. Both traced integration misses had empty ledgers and zero HIGH summaries. 공개 원본 8건에서 추적 4건의 예외 진입·복귀 주소열을 UART와 대조했다. 통합 미달 2건 모두 0청크였다.

Constructed controls. The exact prior cross-cohort HIGH-row mismatch now returned exit 2, empty stdout. Other mixed LOW batch, audit, MEDIUM and marker rows also received exit 2 and empty stdout. The full CLI accepted the constructed 128-chunk ceiling midpoint as a reason-16 miss, exit 1, 2/4. Adding a premature completed batch at that same stage returned exit 2 with empty stdout. The midpoint exercises journal-stage consistency with retained external records. It was constructed on the host, and no original #236 run observed a midpoint miss. 과거 HIGH 모순과 배치·감사·MEDIUM·시각 혼입은 모두 무점수 오류로 분류됐다. 128청크 경계는 호스트에서 만든 단계 검사용 자료이며 해당 시점의 실제 명령 실행을 관측한 자료는 없다.

Focused regression, five mutex-anchor tests, six graph tests and full requirement graph passed. No failed acceptance check, live boot or public write. 지정 회귀 검사·뮤텍스 앵커 5건·그래프 6건·전체 그래프 통과. 신규 부팅이나 공개 변경은 없었다.

To replay, check out the combined tree in a repository containing 317d7365 and run python replay.py PATH_TO_CHECKOUT. 재현 시 위 트리를 체크아웃하고 같은 명령을 실행한다.

[Independent originals, partial-progress controls and reproducible qualification judgments](ordinary-mutex-qualification-corrected.zip) · SHA-256 ffe1a3fb855d3d06cbf54b21ad147aee3028de6f93d71f02c52c82a68bb56ae9.
