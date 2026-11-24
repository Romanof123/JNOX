# #184 paired guest counter-read observation / 게스트 카운터 읽기 관찰

Before implementation, freeze one default-clock share control and one instrumented cold share boot with identical frozen ELFs, pinned Ubuntu QEMU 6.2, the existing 20-second capture limit and ordinary single-vCPU TCG configuration. Preserve both first attempts, partial outputs and the established budget. Build no guest changes. The installed executable exports the v1 instruction translation and execution hooks; this observer needs no register-reading API or privileged scheduler trace.

Resolve return, synchronous entry and IRQ entry CNTPCT_EL0 reads in the actual ELF. Hook immediately before each read and before its next instruction. Record CLOCK_MONOTONIC and CLOCK_THREAD_CPUTIME_ID in each vCPU callback, Linux TID and vCPU index. Keep output bounded in shared file-backed memory. Independently match QMP vCPU TID and the complete ordered journal of closed grants. Return-to-entry is a whole grant, including execution before the timer deadline. It cannot localize host delay to the overrun or identify IRQ assertion.

Require exact matching PCs/opcodes, marker order and count, stable identity, complete guest replay in both arms, positive clock resolutions <= 100 us, every counter-read bracket <= 1 ms, and nonnegative consistent elapsed bounds with the prior fixture's 1,000 ppm allowance. Treat each adjacent pair around one counter read as the in-guest short control; report all widths and per-grant bounds including sync grants. Compare service/budget results for both boots without claiming a universal perturbation bound from a single pair.

구현 전 동일 ELF의 기본 share 부팅과 관찰 부팅 각 1회를 고정한다. 기존 20초 제한, 예산, 첫 실패 기록을 유지한다. 실제 ELF의 복귀·동기 예외·IRQ 진입 카운터 읽기 직전과 다음 명령 직전에서 동일 vCPU 스레드의 벽시계와 CPU 시계를 읽는다. QMP 스레드 ID, 전체 게스트 저널, 명령 주소와 기계어, 순서를 대조한다. 각 짧은 대조 구간은 1 ms 이하, 시계 해상도는 100 µs 이하여야 한다. 1,000 ppm 가정 아래 전체 그랜트의 CPU 외 시간 범위를 산출한다. 만료 이후 구간만의 지연, 실제 IRQ 신호 시점, 과거 실행의 원인은 판정할 수 없다.

## Independent verification / 독립 검증

Use Linux x86-64's native C compiler and the existing weighted-replay Python environment with pyelftools. Keep compiler identity, source revision, build command and shared-object SHA-256 with the external receipt. The plugin uses the public 6.2 v1 ABI subset, without a development header. No guest rebuild is needed. Recover the frozen kernel, user ELF, probe and their **original guest revision** from the existing share receipt. Set shell variables for these four inputs, a fresh absolute output base without commas, and an independently retained session digest for each replay.

```sh
cc -std=c11 -O2 -Wall -Wextra -Werror -shared -fPIC tools/native_weighted_marker.c -o "$OUT/marker.so"
python3 tools/native_weighted_capture.py share default "$OUT/plain" "$KERNEL" "$USER_ELF" "$PROBE" "$GUEST_REV"
python3 tools/native_weighted_capture.py share default "$OUT/observed" "$KERNEL" "$USER_ELF" "$PROBE" "$GUEST_REV" --marker-plugin "$OUT/marker.so"
python3 tools/native_weighted_replay.py "$OUT/plain" "$PLAIN_DIGEST"
python3 tools/native_weighted_replay.py "$OUT/observed" "$OBSERVED_DIGEST"
```

Confirm the same kernel, user ELF, probe, original guest revision and pinned QEMU hash in both launch records, plus the external plugin build hash against the observed session. Record capture and replay return codes individually. The replay reports guest budget misses independently of the marker qualification field. Retain both outcomes even if either threshold fails.

For each guest counter read the before-MRS callback samples wall, thread CPU, wall, and the next executed instruction repeats these reads. At a guest counter read the wall range spans the first wall read of the first callback to the last of the second. The CPU range spans the two CPU samples. Subtract the latest possible start from earliest possible end for the lower elapsed bound, and earliest start from latest end for the upper. Expand by two clock resolutions and clip time lower bounds to zero. Outside-CPU bounds use the host-only fixture rate allowance. A short-control width includes the MRS instruction, callback work and any intervening preemption. It does not bound total observer overhead. A positive outside-CPU lower bound applies only to the corresponding whole observed grant. A zero lower bound permits both zero and positive time. No queue or runnable-state measurement is made.

같은 게스트 입력·QEMU 해시 및 별도 보관한 플러그인 해시를 확인한다. 기본 및 관찰 세션의 게스트 예산 판정과 관찰 시계 검증 결과를 각각 보관한다. 빈 구간에 가까운 짧은 대조값에는 카운터 읽기와 콜백·선점 시간이 포함된다. CPU 외 시간의 양의 하한은 해당 실행의 그랜트 전체에만 적용한다. 실행큐·신호 시점은 이 자료에서 측정되지 않는다.
