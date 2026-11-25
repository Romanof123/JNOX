# Final Functional Safety disposition for POL-010 / POL-010 최종 기능안전 판정

**Proceed to independent Release review.** Functional Safety accepts the retained #79 uncertainty for the finite #337 engineering deployment: independently packaged native applications on one Cortex-A72 QEMU virt CPU, GICv2, TCG, 62.5 MHz counter, quantum 625000, and the captured configurations and workloads at f3be8089034a70bfa4baf35e81ee520f05813526. Record POL-010 as independently verified for these native cases, with scoped Functional Safety acceptance. Release owns its separate decision.

The only outstanding condition in the corrected disposition was recovery of a real native ownership rejection through the production sink in a separately identified control. This condition is satisfied. The first diagnostic guest at 3bdccec1401e66c9f4dcf4219abd8bf24229a58f supplied caller 1 during the actual slot-0 EL0 yield. Predicate 17/10, caller 1, running owner 0 and elapsed 7084 were independently recovered. The counter difference was 568164 − 561080. UART predicate, event 2/8, fault 17 and footer 24 agree with the retained first record, event and architectural context. The saved frame, user SVC at 0x400100 and externally observed ELR 0x400104 corroborate the invocation. Both parked RAM snapshots and PCs at 0x40203abc remained identical across 111088338 host ns. Each of seven ordinary configurations retained its f3be load segments; the six ordinary policy replays and separate legacy first retain their original verdicts.

The first published reader failed its symbol lookup; the original failure remains archived. The fixed public reader at 30e6010e053157329150a99f59134d54a1b5747a recovered the original session, verified both parked frames, and rejected a wrong digest and an altered, resealed entry-counter copy. This closes the prospective first-record recovery condition. Preserve both earlier candidate dispositions and the original #79 failed verdict. Its branch and operands remain unknown. The deliberately triggered diagnostic supplies no operational fault-rate estimate. Physical timing, target interference, deployment workload and product hazard acceptance still require target evidence and the authorized product decision.

**독립 Release 검토로 이관한다.** 기능안전 담당자는 f3be 후보의 지정 단일 Cortex-A72 QEMU virt/GICv2/TCG, 62.5 MHz 카운터, 양자 625000 및 실제 검증한 앱·구성 범위에서 #79의 잔여 불확실성을 수용한다. POL-010에 해당 네이티브 사례의 독립 검증과 한정된 기능안전 수용을 기록한다. Release 담당자의 판정은 별도로 진행한다.

유일하게 남은 조건은 별도 네이티브 대조에서 실제 거부 경로를 거쳐 정지한 최초 기록을 복구하는 것이었다. 슬롯 0의 실제 EL0 양보 때 호출자 1을 의도적으로 전달했다. 독립 판독 결과는 site 17/predicate 10, 호출자 1, 실행 소유자 0, 경과 7084틱(568164 − 561080)이다. UART의 조건·이벤트·오류와 최종 코드 24, RAM 원본, 저장 프레임, SVC 및 외부 ELR가 일치했다. 두 정지 시점의 전체 RAM과 PC도 일치했다. 일반 배포 구성 7건의 적재 세그먼트는 기존 f3be와 동일하며 최초 검증 기록을 보존한다.

최초 공개 판독기의 심볼 조회 실패를 보존했다. 수정 공개 판독기는 같은 원본으로 두 프레임을 확인하고 잘못된 해시와 재봉인한 진입 카운터 변조본을 거부했다. 이로써 선행 보류 조건이 충족됐다. #79의 최초 실패 판정과 과거 후보 판정은 유지한다. 당시 분기와 피연산자는 미상이며 이번 대조로 운용 고장률을 산정할 수 없다. 실기 시간·간섭·배포 부하·제품 위험 수용에는 대상 장치 근거 및 권한자의 별도 판단이 필요하다.

## Reviewed evidence / 검토 근거

- [First parked capture and original reader failure](https://github.com/Romanof123/JNOX/blob/da9ee188d9f37c937e88965ffee1dbb204b96733/docs/evidence/native-proportional-stop-first.en-ko.md). Original session SHA-256 0ca980217348561bddbe0f4738356248298604d510fb11b2c94e014a115a81b5.
- Corrected independent offline packet D:/codex/jnox338-fixed-offline.tgz, verified SHA-256 a772fea453ff771738e19db280ea2965e390bb3dd90241f5060a94b8a1652089. Reviewed report and positive replay output. No new build or guest execution in this disposition.
