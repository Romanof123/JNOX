# POL-015 / #234 prospective plan · 사전 계획

Freeze the first cold run independently for each of exact-lo, exact-hi, over-hi and stagger-idle. Keep ELF, map, raw UART, commands, hashes and failure or timeout from position one. An independent checker must derive the expected jobs, deadlines, counter-grid comparisons, ring sums and first failing row before reporting results. Recheck the three original schemas with their original replay. No implementation or measurement results are claimed here.

네 경우를 각각 독립적으로 최초 콜드 실행하고 실패와 시간 초과도 보존한다. 기존 세 경우의 원래 출력과 판정기를 유지한다. 원본 ELF, 맵, UART와 해시를 보존하고 별도 판정으로 릴리스, 절대 마감, 타이머, 작업 누적값을 재구성한다.

| Hazard / 위험 | Planned requirement and witness / 요구 및 관측 |
|---|---|
| hidden time or reset / 시간·모드 변경 | Explicit checked forward advance only while quiescent. Reject ready/running work and overflow. Idle queries preserve time. HI mode survives idle. |
| shifted deadlines / 마감 이동 | One counter origin and fixed quantum. At 1 enter WFI, at 2 retire physical timer IRQ. Release LO at 2, surviving HI at 3, triggering HI at 4. Deadlines use the original grid. |
| lost carry-in / 잔여 작업 손실 | At 5 discard the partially serviced LO generation. Retain partially serviced HI work across transition. Verify generation, frame continuity and checksum progress at each grant. |
| unsupported approval / 승인 오판 | Report observed response counters, including finite completion, separately from ideal slot results. Enumerate finite bounded idle offsets and carry-in demands separately. External product limits and target evidence remain external inputs. |

측정 응답시간은 승인 한계로 쓰지 않는다. 유한 모델의 입력 범위와 실제 게스트 한 경우를 구분한다.

## Implementation and independent acceptance / 구현 및 독립 판정

Select the existing ordinary AMC build with JNOX_AMC_APP=stagger-idle. Use ordinary_amc_capture.py freeze and run in a fresh case directory, retaining the freeze digest and first/session.json digest outside the directory. Pass the latter digest and directory to ordinary_amc_replay.py. The original three selectors keep wire versions 1/2 and their original acceptance paths. A guest failure or an incomplete first capture is a result and remains attached to its first position.

고정한 QEMU와 ELF로 각 경우를 한 번씩 별도 디렉터리에서 실행한다. 기존 세 경우와 새 경우를 독립적으로 판정한다. 최초 실행 실패도 그대로 보존한다.

The host policy test enumerates all 16 tuples of idle length 1–4 and LO and surviving HI demand 1–2 each. The triggering HI demand is fixed at 2. The guest exercises only (1,2,2). At boundaries 0–6 expect task/generation 0/0, idle, 0/1, 3/0, 1/0, 1/0, 3/0. The transition at 5 discards 0/1 with one grant used. The resumed finite 3/0 reduces its second ring and issues SVC; the original absolute deadline is 8. Report its measured SVC entry minus the scheduled release counter at boundary 3. Other completed service jobs report retirement samples separately. The checker matches private stacks, linked PCs, absolute compares, ring reductions, final products and same-process exception order. External product acceptance requires its own workload admission, hardware evidence and approved limits.

호스트의 16개 조합은 유한 정책 모델이며 실제 실행 측정은 (1,2,2) 한 경우다. 유한 작업의 완료 응답은 경계 3에서 SVC 진입까지의 실제 카운터 차이다. 다른 작업의 슬롯 종료 관측값과 구분한다. 제품 승인 한계와 실제 대상 장치 근거는 별도 입력이다.
