# Ordinary aperiodic server on QEMU virt / 일반 Rust 비주기 서버

CHG-047 / IMP-045 · [Frozen manifests and acceptance criteria](pol-007-rust-server-plan.md)

## English

In the kernel directory build with `JNOX_BOARD=virt`, `JNOX_PROFILE=aperiodic`, and one `JNOX_APP` from `a-ps a-ds a-ss2 a-ss1 b-ps b-ds b-ss2 b-ss1`. Use the repository's bare-metal AArch64 release build without Cargo features. Cold reset the one-core Cortex-A72 virt/GICv2 TCG machine with 128 MiB for each ELF. Save the first UART and QEMU exception log with the ELF hash. The optional `JNOX_APERIODIC_LATE=1` masks IRQs halfway through the first server unit until 1.5 slots after the next boundary. Capture the failing run and cold reset for a new trial.

Four 16 KiB stacks belong to idle (0), producer (1), server (2) and periodic (3). Each job executes as linked Rust after exception return. Kernel code runs with IRQ masked on the outgoing stack. The shared request server reserves one receipt and retires it only after a physical job's completion SVC. The application limits admission to four resident IDs, six outstanding units and four units per request. Candidate admission costs must include one FIFO scan of at most four entries, at most two dated credits, one selection, stack-frame construction, exception save/restore and trace writes. Each selected job makes exactly 256 progress writes and 256 counter samples. An independent time bound for those operations under the selected TCG host remains open. The 1 ms IRQ, 5 ms preparation and residual 45 ms are acceptance limits. Source operation counts alone cannot justify timing admission.

All UART numbers are hexadecimal CNTPCT ticks or unsigned integers. Storage reserves 192 events of ten words, 64 returns of three words, and 16 sets of 256 progress pairs. Records print only after timer disable. Each event starts with kind, slot and sample tick. Seven payload words follow, padded with zeroes.

| Kind | Payload in order |
|---|---|
| 0 setup | F, E, q, contiguous stack base, stack stride, credit capacity, initial frame |
| 1 IRQ | nominal target, interrupted owner, frame address, ELR, whole slots elapsed beyond nominal, retired units, outstanding units |
| 2 submission ack | ID, units, task submit tick, cumulative units submitted |
| 3 arbitration | remaining quota, outstanding units including in-flight, charged, retired, issued ID or zero, number of dated credits, periodic flag |
| 4 switch preparation | outgoing owner, incoming owner, destination frame, destination ELR, nominal slot target |
| 5 SVC entry | owner, saved frame, ELR, x0, x1, x2, x3 |
| 6 completion ack | owner, identity, checksum, worker start, worker finish, latency from task submission or nominal periodic release, retired server units |
| 8 credit | due slot, amount |

`AP RETURN` records destination frame, ELR and sample tick from the restore assembly before its final register loads and ERET. Compare actual ERET externally. `AP STEP` records slot, zero-based step, tick and wrapping result. Per-stream array order resolves ties; join returns to switches in execution order, including the initial idle launch. `AP MASK` records counter reads immediately after masking and before unmasking on the server stack. Zeroes indicate no injection. `AP END` carries the guest verdict (zero for completed guest checks). Replay should verify the 5 ms bound through the return sample and account for the final restore instructions and measurement uncertainty. Separate independent replay, measured maxima, justified bounds, live cold verdicts and real M3 interference are required for release qualification.

## 한국어

커널 디렉터리에서 `JNOX_BOARD=virt`, `JNOX_PROFILE=aperiodic`, 위 8개 중 하나의 `JNOX_APP`을 지정하고 Cargo 기능 없이 AArch64 릴리스 ELF를 만든다. 각 ELF를 단일 Cortex-A72 virt/GICv2 TCG, 128 MiB에서 콜드 부팅하여 첫 UART·예외 기록과 ELF 해시를 보관한다. 결함 실행은 `JNOX_APERIODIC_LATE=1`로 선택한다. 서버 첫 단위의 128단계 뒤 IRQ를 마스킹하고 다음 경계로부터 1.5슬롯 후 해제한다. 미완료 단위를 유지한 실패 기록을 저장하고 콜드 리셋으로 재시작한다.

유휴·생산자·서버·주기 작업은 각각 16 KiB 전용 스택을 쓴다. 표의 원시 레코드를 실제 예외 복귀와 대조하고, 제출부터 단위 완료까지의 시간 및 주기 작업의 명목 릴리스 기준 지연을 계산한다. 작업 단위마다 256회 진행 상태와 카운터를 기록한다. 1 ms IRQ, 5 ms 준비, 잔여 45 ms 실행은 수용 한계다. 저장·복원 및 계측 비용을 포함한 독립 시간 상한과 실제 M3 간섭 근거는 별도로 확보해야 한다.

First runtime verdict: PARTIAL. [EVD-045](evidence/evd-045.en.md) · 최초 런타임 판정은 부분 통과다. [한국어](evidence/evd-045.md)

The standalone default-clock cohort stopped all ten images at the first IRQ with zero charge. The integrated default-clock cohort completed all eight ordinary schedules; both late controls stopped after one charge and zero retirements. The reason for the difference remains unresolved. These are previously published observations. Independent host QA of the replay tool below is pending. [First cohort](evidence/evd-045-v1.en.md) · [Integrated cohort](evidence/evd-045-v2.en.md)

최초 단독 기본 시계 실행에서는 10개 이미지 모두 첫 IRQ에서 차감 없이 멈췄다. 통합 기본 시계 실행에서는 일반 작업 8개가 모두 완료됐고 지연 대조군 2개는 한 단위를 차감한 뒤 완료 처리 없이 멈췄다. 차이의 원인은 미확정이다. 이는 이미 공개한 관측 결과이며 아래 재현 도구의 독립 호스트 검증은 미완료다. [최초 실행](evidence/evd-045-v1.md) · [통합 실행](evidence/evd-045-v2.md)

## Offline replay / 오프라인 재현

From the repository root, run the standard-library checker on one cold boot. Paths below are relative to your working directory. Supply the case actually built, including the suffix `-late` for an injected image, and the selected clock (`default` or `icount`). New captures can use the same UART and QEMU `-d int -D capture/int` streams. An explicit clock alone is a caller declaration. Optional retained metadata checks clock argv, machine settings, capture digests and, if supplied, ELF digest. The checker never opens paths embedded in metadata.

```sh
python tools/ordinary_aperiodic_check.py --case a-ss2 --clock default --uart capture/uart --exceptions capture/int --elf capture/kernel.elf --meta capture/meta.json
python tools/ordinary_aperiodic_check.py --case a-ss2-late --clock icount --uart fault/uart --exceptions fault/int
python tools/ordinary_aperiodic_controls.py --case a-ss2 --complete positive/uart --injected fault/uart
```

Exit 0 means a completed workload or an exercised prescribed fault, named separately in JSON. Exit 1 means an evidenced inhibition, including an early stop before a requested injection. Exit 2 means missing, inconsistent or unsupported evidence. A preparation fault whose last recorded sample precedes the limit cannot independently establish that timing reason. Both clocks use the same FIFO, credit, dispatch, progress and counter rules. A chronology check joins preparation with the assembly return sample and validates each worker's 256 counter/checksum pairs, nominal periodic deadline, server latency and actual receipt. Partial progress survives a failed boundary. An inhibited run can leave observed work in flight.

Supplying `--exceptions` correlates the entire QEMU EL1 IRQ/SVC/ERET sequence with guest records. Without it, the result describes guest return samples. Supplying `--elf` checks file-backed executable PC ranges and reports the ELF hash. Authenticate and map that hash to the selected build independently. A first-boundary inhibition can have identical observable behavior across several applications. The checker cannot infer the compiled case or clock from that UART.

저장소 루트에서 위 명령을 실행한다. 실제 빌드한 케이스와 시계, UART 경로를 지정한다. 결함 이미지에는 `-late`를 붙인다. 정상 완료와 지정 마스킹 결함 관측은 JSON으로 구분하며 종료 코드는 0이다. 시간 초과나 미완료 작업에 따른 억제는 1, 증거 누락·불일치·지원하지 않는 판정은 2다. 결함 주입 전 억제는 미실행으로 표시한다. 선택한 시계와 무관하게 같은 FIFO·예산·상환·단계별 계산·주기 마감을 재현한다. `--exceptions`는 실제 외부 예외 순서를 대조하고, 미지정 시 게스트 복귀 표본만 확인한다. `--elf`는 실행 구간과 해시를 확인한다. 별도 빌드 기록으로 ELF와 케이스의 대응 관계를 확인해야 한다. `--meta`가 없으면 시계는 호출자가 신고한 값이다.

CHG-047 / IMP-045 ties this checker to REQ-002 (dispatch), REQ-005 (temporal behavior) and REQ-023 (resource bounds), DES-003/012 and VER-002/005/023. Raw replay covers the frozen virtual workload and the captured intervals. Product admission, M3 interference and additional execution evidence remain separate. [Frozen acceptance plan](pol-007-rust-server-plan.md)
