# Combined verification of finite workload limits

This is the input to a fresh verifier for VER-002, VER-005 and VER-023. Integration supplies the exact combined Git commit. Keep the first captures and their hashes on persistent storage before looking at implementation details or any other team's verdicts. Serialize all builds, guest runs and reviews. Target one QEMU virt/GICv2 Cortex‑A72 with the default clock and locked Rust 1.95.0.

| Input | Required observation |
| --- | --- |
| `JNOX_PROFILE=multilevel`, `JNOX_APP=mlq-admit` or `mlq-refuse`; each plain and `JNOX_MULTILEVEL_TRACE=1` | The closed four-owner queue has classes 2,1,0,0 in admission order. Q and aggregate allowance are 4,000,000 counts each. Each of the first three owners has 8,000,000 counts of configured total service. ID 3's first-service bound is 24,000,000. Equality admits; one tick less refuses before dispatch. |
| MLQ admitted worker and RAM | ID 2 is interrupted by a real timer before finishing. ID 3 reaches its actual entry, reduces 4,096 samples `1+(i mod 16)`, and publishes its maximum 16. ID 2 resumes, processes a complete block started after receipt and returns. Independently verify all 30 u64 words, the known high products 34,816/382,976 and the equal peer's odd-count reduction of 2,048 per completed block. Reconstruct raw epoch → target entry using charged high work, equal work up to Q, excess and gaps. Match the trace to IRQ, worker frames and actual dispatch. Refusal leaves all runtime fields zero. |
| `JNOX_PROFILE=deadline`, `JNOX_APP=ordinary-edf-supply` or `ordinary-llf-supply`, `JNOX_DEADLINE_OWNER_CAP=3` or `4`; each plain and `JNOX_DEADLINE_TRACE=1` | Q is 100 ms. In every full half-open Q-count window over the finite declared horizon, two possible grid cells and at most two one-shot completions bound entries and real returning owner changes by four. Three refuses before worker frames or dispatch. Four admits. Check earlier-than-estimated completion, endpoints, prior frame restored and final park. Preserve original service sum 89,440, saved A frame and PC, post-restore progress and the jobs' absolute deadlines. |
| Historical cases | Exercise all six original multilevel applications and all eight original deadline applications, plain and trace (28 ELFs). Compare actual runtime outcomes as well as ELF identity, executable text, load images and section sizes against the registered baseline. A first launcher error or missing file retains its position. Record any successful repeat separately. |

Use the repo's public capture helper for the MLQ witness and raw UART acquisition for the capped deadline modes. Seal first results before inspecting application code. Follow with independent small-model and boundary checks, both language matrices and the graph. Finite counter and capacity assumptions apply to these virtual configurations. Record physical and product obligations under the designated release profiles.

## 한국어

통합팀이 고정한 결합 소스와 위 입력표만 새 검증자에게 전달한다. 검증자는 최초 원본과 해시를 영속 저장소에 보관한 뒤 구현이나 선행 판정을 확인한다. 단일 CPU에서 빌드·게스트·검토를 순차 수행한다.

- 엄격 MLQ는 일반·추적 모드별 수용과 거부를 실행한다. 낮은 클래스 작업의 첫 진입 상한과 정확히 같은 24,000,000틱에서 허용하고 1틱 작은 입력은 작업 실행 전에 거부하는지 확인한다. 실제 타이머로 동급 작업이 중단된 뒤 대상 작업이 최댓값 16을 전달하고, 재개한 작업이 이후의 블록 전체를 처리해야 한다. 원시 30워드와 실제 IRQ·문맥으로 서비스 시간과 공백을 재구성한다.
- EDF와 표본 LLF는 각각 소유자 전환 한도 3/4, 일반·추적 모드를 실행한다. 모든 반개구간에서 실제 복귀한 소유자 전환, 일회성 완료, 경계 인접 이벤트, 최종 정지 제외를 확인한다. 독립 서비스 결과 89,440, A의 프레임·PC 복귀와 두 작업의 절대 마감을 유지한다.
- 기존 두 계열 28개 ELF를 기동하고 원본과 적재 영역·코드·실행 결과를 대조한다. 최초 미수집이나 중단은 그대로 남긴다. 실기 및 제품 시간 판정은 각 출시 프로파일에서 추적한다.
