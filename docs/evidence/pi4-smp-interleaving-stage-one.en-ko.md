# PR 356 independent verification / PR 356 독립 검증

Reviewed commit 84587ff607b58a303916e634590cca851c92a7c2 against the supplemental Stage 1 and Stage 2 contract in docs/pi4-native-smp-applications.en-ko.md. The linked worktree was clean. The five firsts were originally acquired on 60475a0ba7c2b1092305043a65d1f6182625a8ed. The committed archive SHA-256 is 85c42a7bf664784ec4c643cc69c9a87579685ae01f20ae71f4947d9b533a157c. No guest, plugin, kernel or application was launched or built in this review.

**Verdict / 판정.** Stage 1 returns 2 at all five positions. Genuine task-owned PPI30 frames establish advancing work on both tasks. None establishes strict cross-core interleaving. The corrected detector can accept a physically feasible positive witness when supplied with independent post-ERET bounds. The exact emulator supports the required public interfaces. Stage 2 still needs a frozen collector, prospective acquisition and its own evidence.

**한국어.** 원본 다섯 건의 두 태스크 모두 실제 타이머 선점과 계산 진척을 확인했다. 엄격한 교차 순서는 다섯 건 모두 미관측(종료값 2)이다. 수정 판독기는 복귀 뒤 실제 EL0 시각 상한이 주어지면 가능한 양성 순서를 판별한다. 원본 QEMU에서 요구 공개 기능을 확인했으며 추가 부팅·플러그인 실행·빌드는 0회다.

| First / 최초 | PPI A/B | Advancing adjacent A/B / 연속 진척 | Weakest kernel-stamp margin / 최소 표식 여유 | Weakest conservative trap-order margin / 최소 예외 여유 | Outcome / 판정 |
|---|---|---|---:|---:|---|
| p1 pair plain | 8/16 | 7/15 | −1,259 | −5,647 | 2 |
| p2 pair traced | 8/16 | 7/15 | −3,188 | −8,786 | 2 |
| p3 remote plain | 8/17 | 7/14 | −1,178 | −6,320 | 2 |
| p4 remote traced | 8/18 | 7/15 | −6,073 | −11,616 | 2 |
| p5 swapped plain | 8/16 | 7/15 | −2,396 | −5,806 | 2 |

Margins in ticks refer to the best reported chain in each position. The position JSON records its four row identities, each signed inequality, measured IRQ/return operands and raw frame digests. An independent RAM reader authenticated 16/17/19/20/16 constituent files and reproduced the complete sets of advancing pairs. C W8 is live in the linked candidate loop. Rust W11 is valid at aligned ELR 0x400050 through 0x40007c, immediately before LDR overwrites it. Each pair matches the same saved/prepared frame at its intermediate dispatch, with continuous owner and an actual programmed PPI comparator. Original private results/exits were 78,498/12 and 70,435/77 on all five, plus 32/32 for both remote positions. Traced p2 and p4 have complete source-row coverage, 26/33 EL0 return lines and at least two possible assignments each. Actual post-return bounds are missing for every position.

Remote p3/p4 separately retain send-before-receipt margins +3,140/−421 ticks and first-clock-to-cell-2 margins +1,198,403/+1,208,071. The separately identifiable earlier SGI tightened neither startup bracket. In p4 the receiver's recorded 3,963,341 precedes the sender's post-send sample of 3,963,686 in their local clocks. SGI selection and the remote deadline survive. Conservative cross-core send-before-receipt time order remains open there.

표의 여유는 각 원본에서 선택된 최선의 사건열 기준이며 단위는 틱이다. 독립 RAM 판독으로 파일 해시, 프레임과 CPU·소유자, PPI 비교기, 두 앱의 증가 후보 쌍을 재확인했다. 추적 두 건의 ERET에는 각각 적어도 두 가지 CPU 배정이 가능하다. 원격 일반·추적의 수신 시각 여유는 +3,140/−421틱, cell 2 기한 여유는 +1,198,403/+1,208,071틱이다. 추적 건의 SGI 선택과 기한 충족은 확인됐으며 시간으로 본 송수신 순서에는 불확실성이 남는다.

## Detector checks / 판독기 검증

Unittest discovery executed all four named committed cases. Independent, different timestamps yielded strict positive continued-user margins [3,6,3,9] across offsets [−1,2], also passing swapped enumeration and CPU reflection. Touching a return or IRQ bound, widening calibration, an intervening row, changing CPU, missing either bound, or a post-return tick outside the local interval yielded no proof. A separate finite-domain oracle evaluated every possible integer IRQ and return time at every common offset for 821 seeded feasible scenarios. Production and oracle agreed on 32 positives and 789 negatives. These are synthetic unit checks. Withholding the sealed p2 exception log or declaring p2 to be p1 independently returned 1. Original p1–p5 retained their own return codes and unmodified data.

커밋에 추가된 네 테스트를 실제 탐색 실행했다. 독립 합성 예제의 연속 EL0 여유는 [3,6,3,9]였고 경계 접촉·행 누락·CPU 변경·복귀 상한 누락은 양성으로 판정하지 않았다. 가능한 시각을 전수 조사한 821개 합성 경우에서 독립 판정과 32개 양성·789개 음성이 일치했다. 추적 로그가 빠졌거나 세션이 다른 입력은 각각 종료값 1이었다.

## Exact model and preflight / 원본 모델과 추가 실행 전 준비

The p2 launch path /home/ywcho/arm-speed-lab-tools/qemu-11.1.3/build/qemu-system-aarch64 currently hashes to 651f3f1b5bb6f388d29f4542f9a3d142f40ab78811807db955903edf81b7a211. The GNU build ID is f2717a7db511bde6a6b5d68032618c682ca8f56b. Its --version reports 11.1.3, -help advertises -plugin, -plugin help exits 0, Meson records plugins = true and CONFIG_PLUGIN is defined. Dynamic ELF inspection counts 65 public qemu_plugin_* exported functions. The same source/build tree has API 7 header SHA-256 335d4e472067e914add598b60d1f7cf0dc029cd48f62f66656d6a88c54d8d9f9. The MG2 precedent uses another executable, 2e465436… .

Per-vCPU translate/execute, memory width/endianness/value, hardware-address RAM/physical queries, discontinuity and idle/resume callbacks are available. The matched TCG source emits instruction instrumentation before execution and the 32-bit store memory callback after its translated store. ARM delivers IRQ/exception discontinuities after updating the target; a synchronous fault may bypass the instruction callback. Normal ERET has no dedicated public return-discontinuity type. Bind execution of the linked EL0-return ERET at 0x02002154 to the first actual EL0 TB/instruction execution on the same vCPU, validate its privilege, PC, owner/root and prepared frame, and reconcile the next exception with the CPU journal. ERET at 0x02002170 returns to EL1 idle.

Before the single additional traced first:

1. Freeze original p2 launch/manifest/session supplied here, the same raspi4b/TCG clock/host scheduling, masks 1/2, priority 2, release cell 0, 62.5 MHz and 1,250,000-tick grid, original -d int, 25 ms terminal check, stop/pmemsave and 120 s watchdog. Seal complete argv, exact model, kernel/image, app ELFs, observer source/compiled object, matching SDK and acquisition/decoder versions.
2. Verify C STR/fall-through b900012b/1100090b at 0x40048c/0x400490, Rust b900014b/17fffff0 at 0x400084/0x400088, VA 0x402000 and p2 result RAM PAs 0x2136000/0x254b000 from their distinct roots. Authenticate instruction identity, register read access or equivalent owner linkage, translation, 32-bit little-endian store and stored values in the actual run. Join store-start, memory access and immediately reached ordinary fall-through on the same vCPU and same EL0 epoch. Invalidate on exception, abort, hostcall, idle, owner change or loss, including a return to the same PC. Independently reconcile total observed increments against the linked initial and terminal private counts.
3. Freeze bounded publication and loss accounting. Sample every 256th verified completed store with C/Rust limits 307/276, separately size IRQ/return streams to 256 journal rows per CPU, and retain attempted/stored/dropped/aborted and first loss. The original runner kills QEMU after stop/pmemsave. An atexit-only exporter cannot guarantee delivery. A bounded per-vCPU shared capture readable after stop, or postterminal acknowledged flush before kill, preserves the original guest schedule. Define invalidation/flush behavior, clock resolution and uncertainty, buffer limits and terminal reconciliation before launch.
4. Require strictly separated widened CLOCK_MONOTONIC brackets on four increasing productive stores in either cross-core order, all inside both independently observed uninterrupted EL0 runs. Use the new first's guest counter operands and single calibrated offset to bound common guest time separately. Preserve the first outcome, including loss or ambiguity, with its own identity.

같은 경로 실행 파일·빌드 ID·도움말·Meson 설정·내보낸 심볼 65개와 일치 소스의 API 7 헤더로 가용성을 확인했다. 예외 진입은 vCPU 식별이 가능하다. 일반 ERET는 별도 복귀 이벤트가 없어 0x02002154 실행과 같은 CPU의 실제 첫 EL0 명령, 소유자·프레임·다음 예외를 연결해야 한다. 추가 1회 전에 원본 추적 실행 조건, 두 저장 위치·값·물리 페이지, 관측기 소스·바이너리와 유실 한도, 호스트 단조 시각 불확실성을 고정해야 한다. 기존 수집기는 RAM 저장 직후 QEMU를 강제 종료하므로 결과 공유 버퍼 또는 종료 전 확인된 기록 회수가 필요하다. 추가 실행에서 얻는 실제 EL0 구간·네 저장·게스트 공통 카운터가 최종 판정 대상이다.

The command scripts, signed per-position margins, full reader JSON, exact binary/configuration/header probes and offline challenge outputs accompany this report. No repository files or GitHub records were edited.