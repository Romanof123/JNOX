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
