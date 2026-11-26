# Native expiry evidence coverage / 네이티브 만료 증거 범위

D is the expiry of accepted sequence 3 and C is the counter operand of the opening CHECK. Two counter readings bracket the store that changed the virtual output from high to low. All three use the same 62.5 MHz clock (16 ns/tick). The offline verifier reads the sealed three-process packet through its pinned decoder and joins the original queue, UART, RAM and output records. REQ-019/024 · IMP-311 · VER-019/024 · EVD-311. [Release criteria #401](https://github.com/Romanof123/JNOX/issues/401) · [Verification #404](https://github.com/Romanof123/JNOX/issues/404)

D는 마지막 수락 허가의 만료 시각이고 C는 CHECK가 판정에 사용한 카운터 값이다. 출력이 1에서 0으로 변한 저장 명령의 전후 시각을 같은 62.5 MHz 카운터로 측정했다. 검증기는 고정된 원본 판독기로 세 프로세스 부팅의 큐·UART·RAM·출력 기록을 대조한다.

| Clock and policy / 시계·정책 | Boots / 부팅 | C minus D (ticks) | Output fall after C (ticks) | Output fall after D (ticks) |
| --- | ---: | ---: | ---: | ---: |
| Default FIFO | 3 | 4,499–17,347 | 1,146–1,695 | 6,189–18,498 |
| Default RR | 3 | 5,664–17,521 | 1,205–1,525 | 7,132–18,731 |
| Instruction count FIFO | 3 | 135–813 | 9–10 | 144–823 |
| Instruction count RR | 3 | 141–956 | 9–10 | 150–965 |

Each range spans the earliest lower bound and latest upper bound over its three boots. Direct independent UART readback reproduced these bounds. In all 12 boots, expiry latched, the virtual output fell inside that CHECK, and the receiver later declined sequence 4. The output fell 144–18,731 ticks (2.304–299.696 µs) after D. These measurements start at permit expiry with the delivery gate open. The 25/5/30 ms closed-gate fault-response test starts at its own fault injection.

각 범위는 해당 부팅 세 건의 하한 중 최솟값부터 상한 중 최댓값까지다. 독립적으로 UART 원본을 직접 읽어 같은 범위를 구했다. 12회 모두 CHECK에서 만료가 래치되고 출력이 내려갔으며 이후 수신기가 순번 4를 거절했다. 허가 만료에서 가상 출력 하강까지는 2.304~299.696 µs였다. 게이트를 닫는 별도의 25/5/30 ms 시험은 결함 주입 시각부터 측정한다.

## Criteria / 판정 기준

The queued-before control requires the whole publication interval before D and the same 44-byte loan at the head on a CHECK exactly at D. For the after-D control the whole publication interval must follow D, the CHECK must occur at D, and the queue must still be empty. Simultaneous publication and CHECK requires both publication bounds and the CHECK operand to equal D. The journal must also determine which operation happened first and the head must agree with that order. A wider interval merely touching or spanning D leaves the publication tick unresolved. Each accepted result also requires actual prior enable, a high-to-low transition inside that expiry decision, retention of the old accepted identity and no later COMMIT or re-enable.

기존에 대기한 사례는 게시 구간 전체가 D 이전이고 정각 CHECK에서 같은 핸들의 프레임이 큐 선두에 있어야 한다. 후행 게시 대조군은 게시 구간 전체가 D 이후이고 정각 CHECK는 빈 큐를 확인해야 한다. 동시 게시 판정에는 게시의 양 끝과 실제 CHECK가 모두 D여야 하며, 저널의 연산 순서와 관측한 큐가 일치해야 한다. D를 포함하는 넓은 구간은 게시 정각을 입증할 수 없다. 해당 검사의 출력 차단도 이전 출력 활성화에서부터 확인한다.

Four frames were queued at D and eight were visible at the actual expiry CHECK. Every C was late. Both exact release criteria remain pending in each clock/policy combination. The separate deterministic component boots and synthetic positive verifier vectors have different scopes. This assessment reuses 12 original guest launches. [Boot cohort and original observations](native-permit-queue-expiry.en-ko.md)

정각에 대기한 프레임은 4건, 실제 만료 검사에서 대기한 프레임은 8건이었다. 정확히 D에서 검사한 부팅은 없었다. 두 시계·두 정책 모두 네이티브 정각 조건은 미검증이다. 이 판정은 과거 부팅 12회의 재판독이며, 구성요소 시험 및 검증기 자체의 양성 벡터는 별도로 집계한다.

## Replay / 재현

Extract the [twelve-boot packet](native-permit-opening-queue.tar.xz) into its own directory. From the repository root run the following with Python 3 and GNU AArch64 binutils. The JSON reports pending release criteria separately from a malformed input, which exits with an error.

[공개 부팅 패킷](native-permit-opening-queue.tar.xz)을 별도 디렉터리에 풀고 저장소 루트에서 실행한다. JSON은 미검증 항목을 표시하고, 불완전한 입력은 오류 종료한다.

```sh
python3 -B tools/permit_expiry_coverage.py /path/to/extracted/arrivals > expiry-coverage.json
python3 -B tools/test_permit_expiry_coverage.py
```

[Machine-readable per-boot results / JSON](native-permit-expiry-coverage.json)
