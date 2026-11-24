# #232 ordinary timed priority inheritance · 설계 v1

[Parent mutex design and requirements](ordinary-mutex-plan.md) · [#232](https://github.com/Romanof123/JNOX/issues/232) · [#49](https://github.com/Romanof123/JNOX/issues/49) · [#21](https://github.com/Romanof123/JNOX/issues/21)

REQ-002/005/023 → DES-003/012 → VER-002/005/023. Single CPU virt, explicit ordinary-chain selector. BASE (1) owns Y and admits LOW (2). LOW owns X and blocks on Y, admitting HIGH (4) and MEDIUM (3). HIGH requests X with a one-shot physical timer deadline of one millisecond. BASE inherits 4 while MEDIUM remains ready. BASE processes a fixed 4 MiB of volatile sensor samples under Y, records progress after each 256 samples and commits count/sum. An actual PPI30 must interrupt this work with partial progress. Remove HIGH only, set its saved result to timeout, recompute LOW and BASE to 2, dispatch HIGH then MEDIUM. LOW receives Y by direct handoff at BASE's release and publishes the checked aggregate under X. HIGH records its timeout without touching X. No spinning on time, artificial sleeps or retries. The timer interval and all work sizes are fixed before capture.

독립 QA는 같은 ELF로 일반 및 외부 추적 최초 콜드 실행을 각각 보존한다. 원본 UART, ELF 해시와 역어셈블리, QEMU 인수 및 종료 사유를 보존하고 IRQ·SVC·ERET, 프레임과 복귀 PC를 대조한다. 준비 큐와 잠금 대기 순서, 두 소유자의 상속 및 하향 복원, HIGH 시간 초과와 LOW 소유권 이전, MEDIUM 실행, 보호된 합계와 부분 진행을 독립 재현한다. 최초 실패 또는 불완전 관측도 보존한다. 실제 보드 시간 상한은 별도 검증 대상이다.

QA must retain first plain and externally traced cold boots, independently replay ready/wait order, both priority changes, PPI comparator and useful progress, match saved and resumed PCs/results to linked code and inspect actual reads and publication. Historical selectors and their acceptance thresholds continue to use their original implementation. First evidence and independent verdict are pending.

## Reproduction · 재현

Start in a clean checkout, using the pinned release toolchain. Create a new output directory before each cohort. From kernel/ build once with JNOX_BOARD=virt JNOX_PROFILE=mutex JNOX_APP=ordinary-chain cargo build --locked --release -j1. Preserve the linked target/aarch64-unknown-none/release/jnox-kernel and disassembly. From the cohort directory run the following with ELF pointing to that image. A parked guest reaches the host timeout. Record both exit statuses, stderr, hashes, QEMU version and command lines.

~~~sh
timeout -k 2s 15s qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg -m 128M -nodefaults -display none -monitor none -serial stdio -kernel "$ELF" </dev/null >plain.uart 2>plain.stderr
timeout -k 2s 15s qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg -m 128M -nodefaults -display none -monitor none -serial stdio -d int -D exceptions.log -kernel "$ELF" </dev/null >trace.uart 2>trace.stderr
python3 "$REPO/tools/mutex_chain_replay.py" plain.uart
python3 "$REPO/tools/mutex_chain_replay.py" trace.uart
~~~

CHAIN RAW 1 retains the original mutex E field order plus remaining ready and wait queue order, encoded oldest first in nibbles (task ID + 1). E rank bytes are the pre-dispatch snapshot. H reports termination reason, decisions and assembly restores. T records frequency, arm, target, IAR, IRQ CTL and CVAL. S identifies stack bounds and frame size. P contains BASE count/sum, LOW published count/sum, HIGH returned timeout and independent MEDIUM sum. BASE exits last with no successor. The terminal header follows that exit. Preserve external trace for independent instruction-level review.

[Independent verification of the first six QEMU boots](evidence/ordinary-timed-chain-first.en-ko.md).
