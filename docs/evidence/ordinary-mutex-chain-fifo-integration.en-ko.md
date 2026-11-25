# JNOX #232 / #236 combined-source independent verification
Source 1bc463d88e5b69763e1268349a57e10685076020 merges 0ae67c38e801b8793be54396ae4d5d0502cae063 and accepted main f06d151bd3be4414f35b873bee48779c43681d27. All eight first cold positions passed on single Cortex-A72 QEMU virt/GICv2, 128 MiB, TCG. Fresh WSL ext4 source archive, Rust 1.95.0, QEMU 6.2.0. Four release/locked ELFs and command script hashed before launch. No guest retries or injected interrupts.

| Selector | First plain | First external exception trace |
| --- | --- | --- |
| ordinary-chain, 1 ms | PASS, IRQ after 1,414 of 16,384 chunks | PASS, IRQ after 2,052 chunks |
| ordinary-fifo, both admission orders | PASS, 412 and 421 | PASS, 412 and 421 |
| ordinary-pi, 100 µs | QUALIFIED, IRQ at chunk 1 | QUALIFIED, IRQ at chunk 7 |
| ordinary-ceiling, 100 µs | QUALIFIED, IRQ at chunk 74 | QUALIFIED, IRQ at chunk 65 |

BASE and LOW inherited priority 4, HIGH timed out on X at PPI30, and both owners fell back to 2. HIGH resumed its saved acquire with timeout 1. MEDIUM completed its sum of 2,176. BASE completed 4,194,304 reads and LOW resumed its saved acquire with success 0 on direct Y handoff, publishing the same count and sum of 35,651,584. Both IRQ PCs were 0x402014d4 in the linked BASE protected load loop, 8,431 and 8,440 counter ticks after compare. Chain replay reconciled all 11 decisions, 12 restores, wait/runnable sets, ownership, external exception order and return PCs. Both emitted CHAIN END.

FIFO replay independently scored both epochs of each boot, including donation/restoration 2→7→2, blocked frames, order of grants and successful returns before writes, 4→41→412 then 4→42→421. Each trace matched actual exception PCs to restores. The two captures ended at FIFO END followed by 100 ms quiet and clean shutdown. All six constructed mutations were rejected.

Legacy controls used unchanged 6,250-tick compares at 62.5 MHz. All had reason 0, 256 checkpoints, MUTEX END, and passed protected-work/continuation/trace replay and the explicit outcome checker. The original 15 s observer returned 124 after terminal for the six parked chain and legacy boots. These are emulator observations. No board timing or family-wide release judgment was made.

140 host tests, 24 scoped Python tests, requirement graph, scheduling evidence matrix and historical POL-014 archive links passed. All eight invalid selectors returned Cargo 101 with build-script rejection. The four positive selectors ran their distinct workloads. The driver records the observation commands. The archived postprocessor’s optional self-copy initially missed its relative input path, so post.sh was copied directly afterwards. All grader, build and test exit statuses were collected.

## 한국어

동일 결합 소스의 네 ELF와 실행 명령을 고정한 뒤 단일 AArch64 virt에서 일반·예외추적 최초 부팅 8개를 순차 실행했다. 8개 모두 통과했다. 연쇄 뮤텍스는 BASE가 보호 작업 1,414/2,052 청크를 끝냈을 때 실제 타이머 IRQ가 들어왔다. 두 소유자의 우선순위 복구, HIGH의 시간 초과 복귀, LOW의 직접 소유권 이전 및 보호 합계 35,651,584를 확인했다. FIFO는 두 부팅 각각 A→B와 B→A의 저장 프레임 복귀 및 보호 기록 412·421을 확인했다. 기존 PI·천장 100 µs 대조 네 개도 원본 판정기로 모두 적격이었다. 호스트 시험 140개, 관련 도구 시험 24개, 그래프·증거 연결 검사가 통과했고 잘못된 선택자 여덟 개는 빌드에서 거부됐다. 실보드에서의 시간 상한은 측정하지 않았다.

[Eight first boots of the integrated timed chain, FIFO and inheritance controls / 통합 뮤텍스 최초 여덟 실행](ordinary-mutex-chain-fifo-integration.zip) · SHA-256 5e623367dca6fe83d321a1377b3d77a95ba2b4c66a00e5e56a1af271aa798177.
