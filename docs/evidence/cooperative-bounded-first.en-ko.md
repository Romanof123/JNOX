# Independent first review / 독립 최초 검토

**Request changes / 수정 필요.** Frozen public source Romanof123/JNOX 667c288b7e8d9dddb395b57b3839e045bb9d2597. Prospective contract 96b268d80c7d9797477de8274d0c887682ebcb16. Registered ten-load baseline a968670a85d3e0770a842c1a64dc67c5aeeaaf59.

With a nonempty burst, post at tick 10 (accepted), 20 (deadline refusal), then 15 (generous deadline). The service accepts the third request. The contract requires rejection of regressed time. Service::admit records last only after enqueue. A rejected high sample is invisible to the next admission and to post-handler clock validation. Record a monotonic high-water mark for all observed samples while the burst is active, preserving queue, peak and reservation on refusals. The dedicated host test clock.rs fails, and both public host tests pass. No source was changed.

대기열이 비지 않은 상태에서 10틱 수용, 20틱 마감 거부 후 15틱 요청을 잘못 수용했다. 거부된 요청의 시각도 활성 구간의 시각 상한에 반영해야 한다. 거부 시 큐·최고 깊이·예약은 보존해야 한다. 독립 회귀 테스트가 실패했고 기존 테스트 두 개는 통과했다.

Before reading the implementation, we captured the plain run and a separate halted QEMU guest's 632 RAM bytes at ELF symbol 0x40219130. The first plain launch failed for missing efi-virtio.rom. The second disabled networking and reached COOP DONE. The monitor captured the first instrumented guest. We inspected raw words before source review. The executable Python oracle was written after source review using the public contract and guest timing. The contract specifies the journal length but leaves field offsets implicit. oracle.txt contains all 79 words grouped as 9 + 5×10 + 3×6 + 2. Guest observations cannot independently certify physical WCET.

구현을 읽기 전 일반 게스트와 별도 게스트 RAM을 확보했다. 첫 일반 실행은 ROM 누락으로 실패했다. 네트워크를 끈 재시도는 완료했다. RAM 원시값을 먼저 판독했고 구현 열람 뒤 공개 계약에 기반한 검산 코드를 작성했다. 양보 전후 값 0→20, 실행 순서 1·2·3, 개별 합·가중합 (9,21), (22,53), (21,58), 총계 (52,132)를 확인했다. 바깥·안쪽 거부 시 큐 수요가 유지됐고 실행 중 남은 예약은 양수였다.

Source review confirms checked finish arithmetic, inclusive deadlines (rechecked on each prefix), capacity and ID bounds, zero-cost guard, active reservation at start+cost, strict expiry after reserved finish, and reset after final accepted work drains. Public controls cover equality, stale samples after accepted posts, active residual, expiry, recovery and overflow. Their existing model explicitly permits rollback after a refused later sample.

코드는 누적 비용 초과, 동일 마감 수용, 접두부 재검사, 용량·ID·0 비용 오류를 처리한다. 실행 중 예약을 반영하고 예약 종료 이후 초과를 검출한다. 비운 뒤 새 구간으로 복구한다. 기존 모델에는 거부된 요청 이후 시각 역행을 수용하는 기대값이 있다.

WSL legacy.sh rebuilds ten normal selectors serially. oracle.py concatenates PT_LOAD file bytes directly from both the archived original ELF and fresh ELF. Eight deadline variants match exactly. coop-events grows from 21650 to 21794 and coop-single from 21074 to 21218 bytes. Both guest transcripts finish after owner yield, both peers, owner resume and two overrun reports. The new Windows-built bounded ELF LOAD is 19163 bytes. bounded.sections distinguishes RX data, BSS and stack. No footprint ceiling or target-board measurement is asserted.

등록 원본 ELF 열 개와 새 ELF를 직접 비교했다. 마감 선택기 여덟 개의 모든 적재 바이트가 일치했다. 기존 협력형 선택기 두 개는 각각 144바이트 증가했고 둘 다 실제 게스트 실행을 마쳤다. 새 구성의 적재 크기는 19163바이트다. 상세 해시는 oracle.txt에 있다.

## Reproduction / 재현

Fresh detached worktree from origin. Windows PowerShell exported JNOX_BOARD=virt, JNOX_PROFILE=fixed, JNOX_APP=coop-bounded and ran python src/tools/build_kernel.py. Copied src/kernel/target/aarch64-unknown-none/release/jnox-kernel to ev/bounded.elf. Under WSL in ev:

    timeout 3 qemu-system-aarch64 -M virt -cpu cortex-a53 -smp 1 -m 128M -nographic -monitor none -serial stdio -kernel bounded.elf > plain.txt 2> plain.err
    timeout 3 qemu-system-aarch64 -M virt -cpu cortex-a53 -smp 1 -m 128M -nic none -nographic -monitor none -serial stdio -kernel bounded.elf > plain2.txt 2> plain2.err
    (sleep 2; echo stop; echo "pmemsave 0x40219130 632 ram.bin"; echo "info registers"; echo quit) | qemu-system-aarch64 -M virt -cpu cortex-a53 -smp 1 -m 128M -nic none -nographic -monitor stdio -serial file:capture.uart -kernel bounded.elf >capture.monitor 2>capture.err

From review root, rustc +1.95.0 --edition=2024 --test ev/clock.rs -o ev/clock.exe and execute; same command with src/tools/coop_bounded_tests.rs for public-tests.exe. Run ev/legacy.sh in WSL, then python ev/oracle.py. The original baseline archive is docs/evidence/deadline-cooperative-legacy-loads.zip at the registered SHA. Extract it under base for reproduction. See tools.txt, windows-tools.txt, bounded.segments, bounded.symbols and individual raw logs.

[Independent first cooperative bounded workload assessment](cooperative-bounded-first.zip) · SHA-256 ad54f563fe2115a1fb1025dd64141e67dba8ca01a6649adb744b08c6991ef188.
