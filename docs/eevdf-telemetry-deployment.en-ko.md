# EEVDF telemetry deployment / EEVDF 텔레메트리 배포

[#331](https://github.com/Romanof123/JNOX/issues/331) · CHG-134 / IMP-134 / EVD-134 reserved · POL-012. Baseline d13f8fd4538b28213d2e79ff69f86e02285df878. PM adopts the exact [#169 finite workload and numerical contract](ordinary-eevdf-plan.en.md). Three linked Rust EL1 entries each own a 16 KiB stack. The target is one Cortex-A72 CPU, QEMU virt/GICv2 with PPI 30, default TCG clock and 128 MiB. Independent EL0 applications, sustained admission, physical WCET and product safety require separate decisions.

## Operator interface / 실행 방법

~~~sh
python3 tools/eevdf_telemetry.py run --out first-run
python3 tools/eevdf_telemetry.py replay --run first-run --session-sha256 DIGEST
~~~

From the repository root on Linux/WSL, use the existing Rust AArch64 bare-metal target, Cargo, QEMU, AArch64 binutils and Python with pyelftools. The run command builds locked release with JNOX_BOARD=virt, JNOX_PROFILE=eevdf and JNOX_APP=telemetry, no bench feature. Record revision, working-tree state, ELF hash, build/tool identities and exact argv. Use a new output directory and retain partial files on failure. Invoke the existing [capture](../tools/ordinary_eevdf_capture.py) exactly once, save the raw UART, RAM, exception/ERET trace, symbol/stack and disassembly records, QEMU argv and session digest. Run the [original strict replay](../tools/ordinary_eevdf_replay.py), save its JSON result and print the paths, digest and verdict. Return failure on unsuccessful build, launch, capture or scoring; distinguish invalid evidence from unobserved required evidence. Reusing a run directory must never overwrite a first position.

The replay command reads the retained artifacts against the supplied digest. It must reject a wrong digest or missing inputs without compiling, launching or rewriting the session. Provide the actual commands, output locations and verdict meanings in both languages. Do not change the kernel or the original scoring policy.

첫 명령은 새 경로에 기존 텔레메트리 이미지를 빌드한 후 단 한 번 부팅하여 원본·해시·재생 결과를 보존한다. 두 번째 명령은 사용자가 보관한 세션 해시로 오프라인 검증한다. 실패와 관측 누락을 구분하고 최초 원본을 덮어쓰지 않는다.

## Frozen workload / 고정 작업

Use F=62,500,000 counts/s and Q=F/100=625,000 counts. Original weights [2,1,1] and requests [2Q,Q,Q/2] select consumer ID 2 first. Batch IDs 0 and 1 process the original ring using index (3n+5×ID) mod 16 and weight 1+(n mod 7). Independently verify each positive completed count, multiple of 64 below 2³⁸, its sum and weighted sum with the 112-reading cycle. The consumer waits before each absolute due time E+8kQ, k=1..8. Only a real PPI30 wakes the task. Its first counter sample comes from resumed task code. Require the complete final tuple (2,128,27028,115128), both batch products after drain and no remaining runnable task.

At the fifth accepted wait charge the old interval, block ID 2 and change B from (weight 1, request Q) to (weight 2, request 2Q) once. Check eligibility, earliest virtual deadline, ties, request remainders, signed-lag-preserving rounding and wake placement. Apply each interval's actual opening weights. Match every selected owner, resumed PC/SP, exception cause and ERET to the journal, including productive resumption of both batch workers.

* Score five overlapping 32Q windows starting E+[0,8,16,24,32]Q. Each must deliver at least 28Q of actual service. Split intervals at window edges and calculate ideal service in exact runnable-weight fractions, excluding gaps. Require |actual−ideal|≤4Q for every one of the 15 signed task/window comparisons.
* For all eight wakes require due ≤ accepted wake ≤ task-owned first-work counter and first−due ≤4Q (2,500,000 counts, 40 ms). Retain each raw sample and nearest-rank p50/p95/max for due-to-first and wake-to-first. Missing waits, ambiguous returns, journal loss, partial outputs or any bound violation leave the corresponding case unqualified.

초기 설정과 다섯 번째 대기의 가중치·요청량 변경을 보존한다. 각 구간 최소 28Q 공급, 작업별 오차 15개 모두 4Q 이하, 기상 8회 모두 요청부터 작업 코드의 첫 표본까지 4Q 이하를 적용한다. 실제 실행 PC, 재개한 작업 및 세 산출물을 원본으로 확인한다.

## Controls and release decision / 기존 대조 및 출시 판정

1. Preserve the controlled selection scope of [EVD-004](evidence/evd-004.en.md). Independently replay [EVD-023](evidence/evd-023.en.md), including its twelve actual choices, ineligible earliest deadline, request/weight changes and two wake observations. Apply its original thresholds; its two response values do not inherit the telemetry mix's 4Q limit. [#56](https://github.com/Romanof123/JNOX/issues/56)
2. Replay [EVD-034](evidence/evd-034.en.md), the saturated fixed 2/2/1, unit-request case with 128 grants and 16 wakes. Reconstruct each owner, ready-weight service/error and wake samples. Replay [EVD-039](evidence/evd-039.en.md), covering 256 grants, 20 wakes, two accepted live changes and two invalid changes preserving the entire state. Score its specified service-error, intervening-grant and response limits. Preserve each named cohort and its original oracle. [#85](https://github.com/Romanof123/JNOX/issues/85)
3. The [original #169 first boot](evidence/ordinary-eevdf-first.en.md), source e203caa997eea8a561c89e59fb41994b64547214, qualified five windows and eight responses, maximum due-to-first 817,052 counts. Its 84 intervals matched 84 ERETs. Archive SHA-256 148e362e5e14c37e1d351df6180f10ae37a5fece35f7ef7ff7655e843c40c452; original session 68cca2efd813872eb02c6868fccbbe4e4e2ae2c22f76a602e4413c755bf0cf16; public session 02c1ac6316ebe22fa54d8b5b40f70f93ddeca356a8ab58fb9a65a56f67b656c6. Use the digest belonging to the actual input. Later [integration](evidence/ordinary-eevdf-integrated.en.md) and [startup comparisons](evidence/ordinary-eevdf-after-native.en-ko.md) established loaded-image equivalence and replayed that session, with zero new guests.
4. Independent Verification freezes release source, settings and retained destinations before the new operator command's first boot. Reopen the original artifacts and independently reproduce all numerical and product verdicts. Check wrong-digest and missing-input refusal and preserve original zero-fairness/zero-response negative controls. Identify meaningful changes to implementations underlying historical evidence, repeating affected controls if necessary. Never substitute an old capture or a subsequent retry for the first release-candidate launch.
5. Independent Release records the verdict on the exact target, earlier controls and current measured workload. Integration records merged provenance in both matrices. PM explicitly decides the [#21](https://github.com/Romanof123/JNOX/issues/21) POL-012 checkbox for this adopted finite deployment.

각 기존 대조군은 원래 허용치와 대상에 맞춰 독립 재생한다. 과거 동일 이미지 대조의 재생과 이번 최초 실행을 구별한다. Release가 실제 산출물·시각·원본을 확인하고 Integration이 양언어 표를 연결한 뒤 PM이 #21에서 채택한 가상 배포의 출시 여부를 명시한다.

## Release decision / 출시 판정 · 2026-11-25

The opening reservation of EVD-134 and baseline `d13f8fd4538b28213d2e79ff69f86e02285df878` record the prospective design. Independent verification tested candidate `6c22596f71e956c9db8b97548dc91de8d79e43cb` against accepted main `93635a0dd59237f7c861c1db6117866d24bd95eb`. After independent Release confirmed both sets of controls, PM approved POL-012 for this finite one-CPU linked EL1 deployment. The first-run receipt is archived at `95c0ba8de2beff919d4c41740c8e0e8767f34059`. [Receipt](evidence/eevdf-telemetry-deployment-first.en-ko.md) · [EVD-134](evidence/evd-134.en.md)

서두의 예약 표기와 기준 d13f8fd는 사전 설계 당시 기록이다. 승인된 main 93635a0과 후보 6c22596을 대상으로 독립 검증을 마쳤다. 독립 Release의 기존·현재 대조군 확인 후 PM은 이 단일 CPU 링크형 EL1 배포에 한해 POL-012 출시를 승인했다. 최초 관측 원본은 95c0ba8에 보존됐다. [EVD-134](evidence/evd-134.md)
