# Native site 17 first-rejection control / 네이티브 최초 거부 대조

This separately labelled build addresses the single pending [Functional Safety observation](../../docs/evidence/native-proportional-safety-corrected.en-ko.md). The build script creates a new detached source tree, retains its patch and uses the existing packager to compile a diagnostic kernel. Only the caller operand at a slot-0 yield changes in that tree. The policy, dispatch, clock, exception handling, rejection recorder, UART and park path come from the checked-out source. The ordinary packaging command has no switch for this fixture. Both private stride processes run the same SDK-built C executable and issue ordinary sched_yield(). The first eligible slot-0 yield reaches the live policy with caller 1 and the real measured service interval. The EL0 issuer remains slot 0.

Linux/WSL, from the committed control revision with the ordinary native SDK toolchain and QEMU available. Use fresh output paths.

~~~sh
rev=$(git rev-parse HEAD)
out=$(mktemp -d)/control
python3 verification/native-proportional-stop/build.py "$out"
pkg="$out/guard-site17-only"
python3 tools/native_proportional_guard.py capture "$out/first" "$pkg/kernel.elf" "$out/yield.elf" "$rev" --peer-elf "$out/yield.elf"
~~~

Use the independent reader revision that provides the capture command. Preserve both revisions when the capture tool comes from another checkout. Preserve its printed session digest and run `python3 tools/native_proportional_guard.py replay "$out/first" SESSION_SHA256`. Retain diagnostic.json, diagnostic.patch, source tree, ordinary application, exact diagnostic kernel, manifest, toolchain and QEMU command. Use the first real FAULT footer to stop capture. Preserve an earlier unexpected fault in place. A timeout, normal exit or truncated footer leaves the positive control incomplete.

Recover the first-record symbol and its actual Option layout from the exact diagnostic ELF; inspect parked physical RAM after the complete UART line `NATIVE EL0 FAULT 24`. Compare RAM with `NATIVE PROP PREDICATE 17 10 1 0 0 0 dt 0`. The predicate identifies caller 1, actual incumbent 0 and unsigned elapsed dt. Recover the service start from the live return/arm data and syscall entry from GUARD_EVENT (kind 2, op 8, result 0). Require dt = entry − actual service start. Retain source TTBR, SP, SPSR, ESR, ELR, timer registers and observed exception/PC evidence; resolve the yield SVC in the exact user ELF, and distinguish the recorded ELR from the SVC instruction address. Retain parked CPU PC and RAM through an independent post-park observation. An injected IRQ, register write or chosen elapsed value invalidates the control. Historical #79 remains unresolved.

**Ordinary binary comparison.** Build the archived f3be ordinary lottery and stride configurations with identical toolchain, paths, flags and user ELF bytes, using the ordinary packager on this clean revision. For each archived configuration compare its corresponding f3be production kernel with its fresh ordinary build using `python3 verification/native-proportional-stop/compare_loads.py ORIGINAL_KERNEL FRESH_KERNEL`. Preserve both binaries and the printed PT_LOAD tuples/digests. Report any mismatch with the affected segment and configuration. Only the explicit diagnostic build uses the retained source overlay. Identity evidence requires measured binaries.

**한국어.** 이 빌드는 별도 worktree에서 0번 EL0 앱의 정상 양보 시 검사 함수에 전달하는 호출자만 1로 바꾼다. 실행 소유자, 실제 측정한 서비스 시간, 예외 프레임, 정책 거부와 UART·정지 경로는 그대로 사용한다. 두 슬롯은 동일한 일반 SDK 앱을 각각의 주소 공간에서 실행한다. 위 명령으로 새 경로에 빌드·최초 캡처 후 세션 해시로 재생한다. 종료선 FAULT 24까지 원시 UART를 보존하고 정지한 RAM·PC를 별도로 확인한다. 정확한 진단 ELF의 심볼과 레이아웃으로 최초 레코드의 site 17, predicate 10, 호출자 1, 실제 소유자 0과 dt를 독립 복구한다. 실제 복귀에서 예외 진입까지의 카운터 차이, 사용자 SVC 주소·ELR·SP·루트·타이머 레지스터와 대조한다. 선행 실패나 불완전한 관측은 보존한다. 일반 lottery/stride 빌드는 동일 입력의 f3be 원본별 PT_LOAD와 비교해 결과를 보관한다. 기존 최초 실행, #79의 원인, 실기 고장률·제품 위험 승인에는 별도 근거가 필요하다.
