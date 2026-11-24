# M1 정적 시간 파티션 디스패치

한국어 · [English](m1-partition.en.md)

CHG-012 · POL-006 · IMP-010 · EVD-010 · #33

4틱 주 프레임 두 개에 동일한 시간표를 적용한다. 틱은 논리 시각이다. 프레임 오프셋 0–1은 A, 2는 B에 할당하고 3은 명시적 유휴 구간으로 둔다.

| 틱 | 기대 실행 |
| --- | --- |
| 0 | A가 진입해 양보 없이 작업한다. |
| 2 | 타이머가 A를 선점한다. B는 차단 상태이므로 유휴 작업이 진입한다. |
| 3 | 명시적 유휴 구간에서 B가 준비 상태가 된다. 같은 프레임에서 중단된 유휴 문맥을 계속 실행한다. |
| 4 | 다음 프레임이 열리면 A가 중단됐던 작업을 재개한다. |
| 6 | 타이머가 양보 없이 실행 중인 A를 다시 선점한다. 준비된 B가 할당 구간에 진입한다. |
| 7 | 명시적 유휴 작업을 재개한다. |
| 8 | 타이머를 정지하고 최종 판정을 출력한다. |

보관한 ELF로 `python3 tools/qemu_boot.py --profile partition --kernel "$ELF"`를 실행한다. 새 게스트 한 대에서 실패를 포함한 첫 블랙박스 판정을 기록한다. 관찰기는 아래 UART 줄 순서와 LF 또는 CRLF를 확인하고 0.1초 동안 추가 출력이 없으면 QEMU를 정지한다.

```text
JNOX M1 BOOT
JNOX M1 READY
PART A FIRST
PART IDLE FIRST
PART B READY
PART A RESUMED
PART B FIRST
PART IDLE RESUMED
PART STOP
PART PASS
```

같은 실행의 리비전, ELF 해시, 실행 인수, UART 원문과 예외 추적을 보관한다. 타이머 IRQ 6회를 링크된 ELF 주소에 대응시켜 중단 PC, 예외 복귀 위치와 디스패치 지점을 확인한다. 해당 소스와 역어셈블리를 읽어 타이머 만료가 논리 틱 2, 3, 4, 6, 7, 8에 어떻게 대응하는지, B가 어떻게 실행 자격을 얻는지, A와 유휴 작업이 어디서 계속되는지 확인한다. UART는 출력 순서의 근거다. IRQ 횟수와 PC만으로는 논리 시각이나 작업의 준비 상태를 판정할 수 없다. 이 계약은 지정된 단일 코어 QEMU 작업 부하에 적용된다. 실기 지연 시간, 메모리 격리, 임의의 파티션 시간표에서의 동작은 별도 증거가 필요하다.

요구사항 REQ-023/002/005에서 설계 DES-012/003, 구현 IMP-010, 검증 VER-023/002/005로 연결한다. 호스트 검사는 짧은 유효 윈도 테이블과 모든 준비 마스크를 독립 오라클과 대조한다. 같은 게스트에서 확인한 예외와 별도로 기록한다.

```sh
(cd kernel && cargo build --release --locked --features partition-bench -j1)
```

[EVD-010](evidence/evd-010.md)

CHG-046 · [일반 Rust 작업의 두 가상 창 배치와 복구 기준](pol-006-rust-partition-plan.md) · [#99](https://github.com/Romanof123/JNOX/issues/99)

POL-006 / IMP-044. v2의 새 정상 콜드 실행은 P 10/10, Q 13/13 경계에서 1 ms를 충족했다. v1은 P 9/10, Q 11/12 경계가 초과했고 Q가 준비된 B 구간을 건너뛰었다. 가상 타이머 통지를 지연한 네 콜드 실행은 주입 경계에서 원래 1 ms 기준에 실패하고 지정된 창으로 복구했다. 그 밖의 지연 초과는 없었다. 실제 작업 마스킹은 미검증이다. [EVD-044 · 통지 지연](evidence/evd-044-notice.md) [EVD-044 v2](evidence/evd-044-v2.md)
