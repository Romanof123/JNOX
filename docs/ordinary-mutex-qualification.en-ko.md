# Ordinary mutex release qualification / 일반 뮤텍스 릴리스 판정

For [#241](https://github.com/Romanof123/JNOX/issues/241), run the opt-in offline command from the repository root. 기존 자료를 판정할 때 저장소 루트에서 다음 명령을 실행한다.

```sh
python tools/qualify_ordinary_mutex.py COHORT.zip
```

Supply one original #236 capture ZIP containing both ordinary-pi and ordinary-ceiling. In each directory plain.uart and trace.uart are distinct original launches. exceptions.log belongs to the traced launch. SHA256SUMS must cover the inputs. The guest H row identifies the protocol and external IRQ/SVC/ERET records must match the traced UART. The standalone legacy replay and its original archive keep their existing interface. 각 ZIP은 PI·천장 일반/추적 최초 실행 네 건을 담는다. 각 입력의 해시와 게스트 구성값, 추적 예외 기록을 확인한다. 기존 재생 명령은 그대로 사용한다.

Each complete valid cohort prints one line per fixed position, in the table order, as `POSITION QUALIFIED` or `POSITION UNQUALIFIED reason=16`, followed by `qualified=N/4`. Exit 0 requires 4/4. Exit 1 retains every observed miss. Exit 2 means invalid inputs, missing or repeated evidence, or failed replay. Invalid cohorts report a diagnostic on stderr and receive no score. 유효한 자료는 아래 순서로 각 위치 판정과 총점을 출력한다. 종료 코드 0은 전부 적격, 1은 실제 관측 미달, 2는 자료 오류이며 오류에는 점수를 매기지 않는다.

| Position / 위치 | First feature / 최초 기능 | First integration / 최초 통합 |
|---|---|---|
| PI/plain | QUALIFIED | QUALIFIED |
| PI/traced | QUALIFIED | UNQUALIFIED reason=16 |
| ceiling/plain | QUALIFIED | QUALIFIED |
| ceiling/traced | QUALIFIED | UNQUALIFIED reason=16 |
| Score and exit / 점수·종료 | 4/4, 0 | 2/4, 1 |

Frozen [feature archive](https://github.com/Romanof123/JNOX/blob/317d7365ad966441453eabb38aeab1c78e751f7b/docs/evidence/ordinary-timed-chain-first.zip) SHA-256 `001d81b80f4e5635676bd6c2f13b31f03d4077e9ff132f226deec726f79353e7`; [integration archive](https://github.com/Romanof123/JNOX/blob/317d7365ad966441453eabb38aeab1c78e751f7b/docs/evidence/ordinary-timed-chain-integration.zip) SHA-256 `fa89510c77757edbcbd70c300c69663f5e7220d31a1303f84015c6207db362fd`. Both traced integration IRQs arrived before protected work, with zero chunks. 통합 추적 두 건은 보호 작업 전 IRQ가 발생해 각각 0청크로 미달했다.

The [normative plan](ordinary-mutex-plan.md) fixes the 100 µs stimulus, release during protected work and ceiling release before the 128-chunk yield, decision/restore/trace checks and committed result. POL-014 connects REQ-002/005/023, DES-003/012 and VER-002/005/023 under [#49](https://github.com/Romanof123/JNOX/issues/49), [#232](https://github.com/Romanof123/JNOX/issues/232) and [#6](https://github.com/Romanof123/JNOX/issues/6). 기존 100 µs 자극, 작업 중 IRQ, 천장 128청크 전 조건, 복귀와 최종 데이터 검증을 적용한다. 결과 범위는 원본 단일 CPU 가상 실행이다.

## Correction contract / 관측 미달 자료의 일관성

The first processing receipt and both original cohorts remain the baseline. A reason-16 stop occurs inside the one-shot IRQ handler, before the newly released HIGH or MEDIUM can return to task code. Their result, audit read, sum and markers, and the HIGH handoff marker must therefore retain their initial values. A selected successor has not yet executed. 사유 16은 IRQ 처리 중 발생하므로 방금 준비된 HIGH·MEDIUM은 아직 실행되지 않았다. 두 작업의 결과·감사 읽기·합계·시각과 HIGH로의 잠금 이전 시각은 초기값이어야 한다.

Validate LOW against its actual progress and released locks. Before the last chunk its batch, commit, audit and audit marker are unwritten. After all chunks, allow the documented intermediate batch stores and a completed commit, and allow a ceiling audit after returning from the Y unlock. The post-acquire marker can precede the first chunk. Any nonzero task marker must occur within the applicable task execution interval. An impossible summary or marker yields exit 2 with no score. A coherent out-of-window IRQ retains exit 1. LOW는 실제 청크 수와 잠금 해제 순서에 따라 검사한다. 마지막 청크 전에는 게시·감사 결과가 초기값이어야 하며, 마지막 청크 뒤에는 단계별 쓰기와 게시 완료가 가능하다. 천장 모드의 Y 해제 복귀 뒤에는 감사를 허용한다. 첫 청크 전에도 획득 복귀 시각은 존재할 수 있다. 불가능한 결과나 시각은 종료 2이며 일관된 관측 미달은 종료 1이다.

Regression expectations / 회귀 기대값. Reuse the two original cohorts for 4/4 and 2/4. An integration miss with a completed HIGH snapshot, a premature LOW commit, or a later task marker receives exit 2. A journal-consistent later partial LOW stop may retain its own progress. 원본 점수는 유지한다. 미달 기록에 다른 실행의 HIGH 결과, 이른 LOW 게시값 또는 후속 작업 시각이 섞이면 자료 오류로 판정한다. 뒤늦게 도착한 IRQ의 실제 부분 진척은 허용한다.
