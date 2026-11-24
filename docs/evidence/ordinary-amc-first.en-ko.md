# POL-015 independent first boots / POL-015 독립 첫 부팅 검증

**Exact LO passed. Exact HI and one over HI failed the fresh LO result timing check.** All three original guests reached their footer. The first observation of the fresh generation preceded b6 in each transition run, but both final results arrived after b6. The declared deadline of the fresh generation was b8. The failed requirement was the earlier b6 observation specified in the workload contract.

**Exact LO는 통과했고 Exact HI와 One over HI는 복귀한 LO 작업의 최종 결과가 늦어 실패했다.** 두 전환 실행에서 새 LO 세대의 첫 표본은 b6 전에 기록됐다. 계속 갱신된 최종 결과가 b6을 넘었다. 해당 세대의 상대 마감으로 계산한 b8과 실험 계약의 결과 관측 시한 b6은 별개다.

Contract / 계약 74f1d5a7545489069d3677275ab6c6a864b2ec79 (#163). Detached runtime / 실행 커밋 c4a63bebc3b773550c630522ba3f62cd9be1a526, tree 5cc8bb0e69a24849316a2b118ed3cb1fd5e4ec8a. Frozen before source review / 소스 확인 전 고정한 protocol.txt SHA-256 a76fec3b9c0681b6a1f1b4c0fad8e434bc02691c80bf1730edaea0542ede5287. Windows MSVC Rust 1.95.0, kernel directory, JNOX_BOARD=virt JNOX_PROFILE=amc JNOX_APP per case, cargo build --release --locked -j1. Each ELF cold-booted once on WSL Ubuntu 22.04.3 ext4 in QEMU 6.2.0, single Cortex-A72, GICv2, TCG, 128 MiB.

| Case / 실행 | Owners / 슬롯 | b2 | IRQ | Commit | First resumed sample / 복귀 | x−b2 at 62.5 MHz | Final fresh LO vs b6 / 최종 결과 |
|---|---|---:|---:|---:|---:|---:|---|
| Exact LO | 0,1,0 | — | — | — | — | — | No transition / 전환 없음 |
| Exact HI | 0,1,1,1,3,0 | 6297527 | 6308418 | 6320873 | 6326732 | 29205 ticks, 467.280 µs | 18808034 > 18797527 by 10507 ticks |
| One over HI | 0,1,1,1,3,0 | 6301279 | 6310882 | 6320307 | 6324751 | 23472 ticks, 375.552 µs | 18810504 > 18801279 by 9225 ticks |

The OS calls AmcRuntime::start_slot to select each owner. Exact HI retired job 1 at 12558806 before b5=15672527 and job 3 at 15685491 before b6=18797527. One over HI retired the third grant with one unit unserved, denied the fourth and dispatched job 3 at the same modeled boundary. Job 3 retired at 15687193 before b6=18801279. Both discarded LO generations remained at zero samples. See each independent JSON for every generation, stack interval, per-slot first/last count, checksum and linked exception sequence.

OS가 AmcRuntime::start_slot으로 매 슬롯의 소유자를 선택했다. Exact HI에서 작업 1과 생존 작업 3은 각각 b5, b6 전에 종료됐다. One over HI에서 작업 1은 세 번째 할당 후 요구량 1이 남았고, 네 번째 할당을 거절한 같은 경계에서 작업 3이 실행됐다. 폐기한 두 LO 세대의 표본 수는 모두 0이었다. 각 independent JSON에 세대별 스택, 표본 수와 합계, 복귀 및 인터럽트 주소를 수록했다.

Independent arithmetic verified both observations of every slot and each FINAL against the rotated-ring formula. The observer paired 3, 6 and 6 ERETs and IRQ ELRs with UART records from the same QEMU process. The 140-byte linked ordinary_amc_worker starts at 0x40202028 (LO) or 0x4020202c (transition). Exception traces confirm the PCs. Generation and SP attribution uses the guest's saved-frame records. The replay controls changed only in-memory copies of the accepted LO input. Changing owner, checksum, final generation or ERET was rejected. Removing the trace yielded inconclusive. Single virtual runs establish no physical-board or product-wide bound.

독립 계산으로 각 슬롯의 첫 표본과 마지막 표본, 최종 결과의 순환 입력 합계를 대조했다. 동일 QEMU 프로세스의 예외 기록에서 각 실행의 ERET와 IRQ ELR을 각각 3쌍, 6쌍, 6쌍 확인했다. 세대와 스택 주소는 게스트 프레임 기록으로 식별했다. 통과 기록의 메모리 복사본에서 소유자, 합계, 최종 세대, ERET 주소를 개별 변경하면 모두 거부됐고 예외 기록을 제거하면 판정 불가였다. 물리 보드와 제품 전체의 시간 상한은 검증 대상에서 제외했다.

## Offline replay / 오프라인 재검증

Install pyelftools, extract the pack, and use Python 3 with the exact candidate checkout. For each case run python CHECKOUT/tools/ordinary_amc_replay.py CASE HASH, using the matching session digest below. The two rejections are expected. Run python audit.py and python controls.py CHECKOUT/tools from the extracted directory for independent arithmetic/trace comparison and the six replay controls. 각 CASE의 ELF 및 최초 세션 파일은 원본 바이트로 보존했다. SHA256SUMS로 압축 해제한 파일을 대조할 수 있다.

| Case | SHA-256 of first/session.json |
|---|---|
| exact-lo | 8d57ea15321f8ceaa9a21900b98d82148e8a2486e51a9742e9ce803e01a0a914 |
| exact-hi | 1bff0f4270a2c52ddb84c70d265c0b6405878eb5d936450d086feb960407e9b2 |
| over-hi | 3399b79769703ccff1558b6f41b4f059936a08e9bfa20eab866f699fd9d7f6e1 |
