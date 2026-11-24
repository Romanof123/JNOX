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
