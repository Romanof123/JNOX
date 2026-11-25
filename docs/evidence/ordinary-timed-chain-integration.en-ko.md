# #236 / #232 prospective main integration · 병합 검증

**New chain 2/2 qualified. Historical controls 2/4 qualified.** Both traced controls stopped with reason 16 and zero completed chunks. The six-boot acceptance criterion remains unmet. No retries. All six parked guests reached host timeout 124.

Integration commit 06fbc19906d7db4971f767b40fdf1236aae1be4b, tree 4fe635849da3d607ea399744d9a2ff848820280d. Public main bd897ccd121021dcabb73b5e1fdf6c74b6a970a8 and PR head 884c9fe4eeca33977e238a09508f1de232855530 match both parents. Each selector built on first attempt through the documented Cargo entrypoint using Rust 1.95.0. WSL Ubuntu QEMU 6.2.0, single virt Cortex-A72 TCG, serial builds and guests.

| Workload | Plain | External exception trace |
|---|---|---|
| Ordinary chain | IRQ after 2,101 chunks, 9,916 ticks beyond comparator | IRQ after 1,365 chunks, 95,758 ticks beyond comparator |
| Ordinary PI | IRQ after 57 chunks, completed 256 | reason 16, zero chunks |
| Ordinary ceiling | IRQ after 18 chunks, completed 256 | reason 16, zero chunks |

Both chain replays verified the timed waiter removal, two priority restorations, direct handoff, protected count 4,194,304 and sum 35,651,584, HIGH timeout 1 and MEDIUM sum 2,176. The traced exception PCs matched the saved and restored guest records. All three ELF hashes match the earlier archived boots. Reason 16 occurred before protected work in the traced controls. Historical replay prints UNQUALIFIED and exits zero for this state. Timing values describe these guest samples.

All four invalid Cargo configurations failed with the expected selector-specific diagnostics (fixed/ordinary-chain, mutex/fixed-fifo, unset profile/ordinary-chain, mutex/ordinary-chain with rust-prepared anchor). Host scheduling suite 137/137, scoped Python tests 24/24, requirement graph exit 0, scheduling matrix 18 policies and zero findings. Both POL-014 rows resolve via the bilingual first report to the CRC-checked original six-boot ZIP SHA-256 001d81b80f4e5635676bd6c2f13b31f03d4077e9ff132f226deec726f79353e7. Run python3 check.py on the extracted directory and read the individual verdicts. Commands, ELF/disassembly, UART, exception traces, build diagnostics and regression outputs are included.

**한국어.** 실제 병합 트리에서 일반 체인은 최초 2회 모두 통과했다. PI와 천장 대조군은 일반 실행만 각각 256청크를 완료했고, 두 추적 실행은 0청크에서 사유 16으로 종료되어 총 2/4회만 적격이다. 재시도 없이 모든 최초 결과를 보존했다. 체인의 두 소유자 우선순위 복원, 시간초과 복귀, 직접 소유권 이전과 보호 합계를 재생으로 확인했다. 잘못된 선택 조합 네 건 모두 빌드에서 거부됐다. 호스트 회귀 137건과 범위 지정 검사 24건, 그래프·정책 행렬 검사가 통과했고 한영 POL-014 행은 동일한 최초 원본을 가리킨다. 검증 범위는 단일 가상 CPU이다.

[First integration captures and replay](ordinary-timed-chain-integration.zip) · SHA-256 fa89510c77757edbcbd70c300c69663f5e7220d31a1303f84015c6207db362fd.
