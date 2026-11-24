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
