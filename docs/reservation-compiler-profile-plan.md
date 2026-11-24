# CHG-064 · Reservation compiler profile comparison / 예약 컴파일러 프로필 비교

[Issue #123](https://github.com/Romanof123/JNOX/issues/123) · [Compact source #138](https://github.com/Romanof123/JNOX/pull/138) · [Compact wire contract](reservation-compact-handoff-plan.md) · [Published EVD-053 v3 English](evidence/evd-053-v3.en.md) · [공개 EVD-053 v3 한국어](evidence/evd-053-v3.md) · [EVD-053 v2 English](evidence/evd-053-v2.en.md) · [EVD-053 v2 한국어](evidence/evd-053-v2.md)

## Question and decision

Can one release optimization setting reduce measured cold IRQ-to-precommit and source-read-end-to-precommit work sufficiently for ordinary W/S to finish under the existing guards? The retained CHG-059 REPORT.md measured compact v3 at 4/8 and matched v2 at 5/8. Historical v2 reached 1/8 and both v1 cohorts reached 0/8 each. Keep all four cohorts separate. Three v3 openings committed and read back the cutoff but failed before selection (2/5). A fourth reached a late Rust marker (2/2). The compact commit took 2,721–3,658 ticks while source-read-end to precommit alone took 15,522–22,627 ticks across all eight v3 openings (minimum CBS-saturated/trace). Source includes live frame, identity, progress, timer and bank checks. Earlier paired launches compared different implementations at size optimization. No equivalent paired profile comparison appears in those manifests.

In the retained size-built aligned-wake ELF (SHA-256 7cfa99d34389ee71a8e55bc65e1e89dfd75f2e4abcdea017ffef51132e89fbb4), the IRQ at 0x402012c8 reserves 0x160 bytes and saves six register pairs x29/x30 through x20/x19. Live acquire reads precede the sampled publication bracket (counter 0x402019e4, STLR 0x40201a24, LDAR 0x40201a28, counter 0x40201a3c). Cutoff write/readback follow at 0x40201af4/0x40201b00. Inlining, register allocation, block placement and cold translated instruction footprint could alter work in this interval in either direction. Addresses belong only to this ELF. Static instruction counts cannot attribute elapsed ticks.

## Freeze before the first run

1. Build both profiles from exact b9a247c68c3ab2544f18158c8dd13fc15cab6428. Pin decoder 54218be8e6e06382cd491bc2e3af9f61705d4f0c. Build from kernel/ with cargo +1.95.0 build --locked --release -j1; rustc 59807616e1fa2540724bfbac14d7976d7e4a3860, host x86_64-pc-windows-msvc, LLVM 22.1.2, target aarch64-unknown-none, target-cpu=cortex-a72, panic=abort. Pin Cargo.lock, linker, target libraries, source tree, build inputs and compiler version/hash, exact argv and environment. JNOX_BOARD=virt, JNOX_PROFILE=reservation and JNOX_APP selects the documented six W/S/R images. Build sequentially in clean identical paths/configuration and archive each before reuse. S uses the existing release opt-level=s with override absent. O2 uses only CARGO_PROFILE_RELEASE_OPT_LEVEL=2. Preserve all other release defaults and flags; record verbose invocations. Keep the tracked production profile untouched. Stop for configuration drift or unreconciled size-ELF baseline differences. Publish ELF, loadable segments, symbol/section map and hashes by source, selector and optimization. A plain/trace pair for a selector must use identical bytes.
2. Before any launch compare executable/constant bytes, load image, data/BSS, stacks, static pools and kernel RAM per ELF. Account separately for the measured 128+128+264+184 = 704 bytes for bank/witness/receipt objects, plus trace, remaining state, padding and stack. Identify aliases to avoid double counting. Report deltas against S and REQ-009 proposed minimal 256 KiB code+constant and 2 MiB RAM reference ceilings. Report reserve and unknown stack high-water separately. An instrumented QEMU map cannot establish physical minimal-profile qualification.
3. Disassemble each ELF and tie addresses/symbols to its hash. Record IRQ frame/spills, callee-save and vector frame layout/restoration, saved ELR/SPSR/x19/TPIDR, worker marker/return instruction sequence, stack bounds, reachable validation branches, barriers, counter samples, real IAR/CTL/CVAL and cutoff readbacks, single publication/readback and separate failure stops. Inspect for aggregate copy/calls on the actual first IRQ path. Preserve cold boot epoch, masks, admission/refusal, per-slot budgets, worker code and progress recurrence, RAW3 receipts and terminal-format overhead; R retains RAW1. Stop before launch if semantics, measurement sites or provenance are unresolved.
4. Freeze 20 first launches as ten adjacent S/O2 pairs on one serialized native Linux filesystem host. Odd pairs run S first, even pairs O2 first. Pair order: aligned-W/plain, CBS-W/trace, aligned-S/trace, CBS-S/plain, aligned-W/trace, CBS-W/plain, aligned-S/plain, CBS-S/trace, aligned-R/plain, CBS-R/plain. Record exact wall-clock start/end and separation within every pair, host/load and interruptions. Use one fresh QEMU process each with default clock, 15 s per attempt, no warm-up/cache eviction, retry substitution or co-running guest. Pin QEMU 6.2.0 executable SHA-256 0a63468782d096485932acf6aa0a69d358b3e3a6f68f655e960e9f5e3fc578d8 and flags -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg -m 128M -nodefaults -display none -monitor none -serial stdio -kernel ELF. Trace arms append -d int -D LOG. Archive raw UART, stderr, exception logs, meta, command, process status, ELF and hashes in separate directories for every ordinal. Retain timeouts/incomplete captures; no relaunch to fill missing data.
5. Report each adjacent pair separately in ticks. Keep plain and trace strata separate; give S and O2 values, signed difference only when both endpoints exist. Record boot/config/epoch, idle mask/ready, IRQ–E, IRQ–source end–pre–post–arm–selection–assembly return–Rust marker, terminal decode/result and later upper service per grant/allocation. Preserve source reads and instrumentation overhead and missing stages. For every failure give earliest measured failing guard, status, reason, actual operands, stage bits, bank/debit, selection, actual progress, return and unobserved downstream intervals. For successful W verify 34 slots with neighbor service; for S verify 10 slots, four grants each and exits. Replay original ledger, stack, recurrence, return and timing checks; external exception order is available in trace arms only. Each R must refuse 9/8 before arming/dispatch in each profile.
6. Offline negatives use copies with swapped/missing/duplicate arm ordinals, mismatched ELF hash or plain/trace attribution and swapped/missing/duplicate boot/terminal tag 24 roles. Require rejection, retain original decoder failure, corrected decoder result and independent replay. A missing stage is unobserved. A guard failure, unsupported readback, ambiguous pairing, integrity error, incomplete capture, cap breach or fewer than 8/8 qualified normal runs rejects qualification for that arm. If precommit shrinks only in some matched cases, another stage dominates, footprints regress or outcomes mix, retain the mixed/rejected hypothesis and leave the default intact. Even a consistent gain and 8/8 here establishes only this finite QEMU experiment; any production change requires separate approval, footprint and physical timing evidence. No changed deadline, delay, injected fault or suppression.

## 한국어 실행·판정 기준

같은 소스에서 크기 최적화와 O2의 최초 IRQ 검증 구간을 비교한다. 기존 v3 4/8, 동시대 v2 5/8, 이전 v2 1/8, 두 v1 집단 각 0/8은 각각 보존한다. v3 세 건은 커밋·컷오프 되읽기 뒤 선택 전 실패했고 한 건은 Rust 표식이 늦었다. 실제 바이너리의 352바이트 IRQ 프레임과 6쌍의 저장 레지스터, 메모리 순서 및 타이머 명령 주소는 위에 기록했다. 최적화가 분기 배치·레지스터 할당·콜드 번역량을 바꿀 가능성을 검정하며 사전 성능 향상을 전제하지 않는다.

두 빌드 모두 위 고정 소스·컴파일러·링커·타깃·보드·선택자·명령을 사용한다. O2만 환경에서 최적화 수준을 2로 바꾼다. 각 ELF의 해시·적재구간·맵과 디스어셈블리를 따로 남기고 프레임 복원, 가드, 카운터, 타이머, 커밋, 실패 경로와 계측 의미를 대조한다. 704바이트의 네 객체 외에도 전체 코드·상수·RAM·스택·풀과 여유를 REQ-009 참조치에 비추어 기록한다. 설정·동작·출처를 확인할 수 없으면 실행 전에 중단한다.

위 순서의 인접 쌍 10개를 한 호스트에서 교대로 실행한다. 각 프로필 W/S 8회와 R 2회이며 원시 기록·ELF·예외 추적·실행 간격·프로세스 결과를 보존한다. 평시/추적·정책·작업별 짝의 모든 구간과 실제 최초 실패를 따로 보고한다. 누락된 값은 미관측으로 기록한다. 실행 순번·해시·태그 24 역할 오류는 별도 사본에서 거부한다. W 34슬롯의 이웃 서비스, S 10슬롯 각 4회와 종료, 원장·복귀·시간, R의 9/8 사전 거부를 요구한다. 정상 8/8 미달·상한 위반·귀속 불명은 해당 판정을 거부한다. 일부 구간만 개선되거나 결과가 엇갈리면 기본값을 유지한다. 실제 보드의 시간 상한과 양산 승인은 별도이며 가드·기한·관측 비용을 완화하지 않는다.


### Frozen launch ordinals / 고정 실행 순번

S = release opt-level=s (existing reference / 기존 기준), O2 = release opt-level=2. The same selector ELF serves both modes within each compiler arm. 각 컴파일 설정 안에서 동일 선택자의 일반·추적 실행은 같은 ELF를 사용한다. The retained size-built aligned-wake reference / 기존 aligned-wake 크기 최적화 기준 ELF SHA-256 is 7cfa99d34389ee71a8e55bc65e1e89dfd75f2e4abcdea017ffef51132e89fbb4 at source b9a247c68c3ab2544f18158c8dd13fc15cab6428. 전체 v3 정상 대상 8회에서 source_end_to_pre 실측 범위는 15,522~22,627틱이며 최솟값은 cbs-saturated/trace였다.

| Ordinals / 순번 | Selector / 선택자 | Mode / 모드 | Profiles in order / 순서 |
|---|---|---|---|
| 1, 2 | aligned-wake | plain | S, O2 |
| 3, 4 | cbs-wake | trace | O2, S |
| 5, 6 | aligned-saturated | trace | S, O2 |
| 7, 8 | cbs-saturated | plain | O2, S |
| 9, 10 | aligned-wake | trace | S, O2 |
| 11, 12 | cbs-wake | plain | O2, S |
| 13, 14 | aligned-saturated | plain | S, O2 |
| 15, 16 | cbs-saturated | trace | O2, S |
| 17, 18 | aligned-refused | plain | S, O2 |
| 19, 20 | cbs-refused | plain | O2, S |

Only one of eight size-optimized normal boots completed. None of eight O2 boots completed. Both profiles refused both overloads. Retain the production size setting and 1 ms guard. [CHG-064 · EVD-053](evidence/evd-053-compiler.en.md)

일반 부팅은 크기 최적화(S) 8건 중 1건, O2 8건 중 0건이 완료됐다. 두 설정 모두 과부하 2건씩을 거절했다. 기존 크기 최적화와 1 ms 경계 검사를 유지한다. [CHG-064 · EVD-053](evidence/evd-053-compiler.md)
