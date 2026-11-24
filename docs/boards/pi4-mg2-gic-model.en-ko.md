# Pi 4 MG2 · Observe GICv2 delivery on the pinned QEMU model

This diagnostic asks how far CPU1’s opening SGI 1 travels toward CPU0. Stage 1 saw CPU1 post 0x18001 in five early finishes and observed a sixth, clustered position resume after M’s last checkpoint on CPU0. The missing observations are the model’s distributor write, winning pending candidate and IRQ-line decision. The strict migration result remains 0/6. [Stage 1 receipt](../evidence/pi4-mg2-terminal-first.en-ko.md) · [Original contract](pi4-smp-migration.en.md) · [#47](https://github.com/Romanof123/JNOX/issues/47)

## Frozen comparison

One pair, in order, on the archived **clustered** image. Position 1 uses the Stage 1 controls with GIC event logging off. Position 2 enables six existing QEMU trace events. Both use the same physically identical guest, emulator and one-second post-terminal RAM capture. Retain both first attempts, including failures. These are two additional model observations for the specified regime. There is no new six-position qualification cohort.

| Input | Frozen value |
|---|---|
| Guest raw SHA-256 | `cc3dbb8ececa7f00c01164b716310596d73bdad1c8eed49753f9f3a5ea60ce79` |
| Model SHA-256 | `3ae2c079d4491362fece2abb641f0ff17d88e9008c9fcd13600f249b9bd1c862` |
| Model GNU build ID | `01d47c7221d14fdfe82890c76e1056d4cbfe1ee8` |
| Execution | raspi4b, MTTCG, four model CPUs, CPUs 0/1 online, original 30 s limit and 1 s terminal quiet |
| New observed events | `gic_dist_write`, `gic_update_bestirq`, `gic_update_set_irq`, `gic_acknowledge_irq`, `gic_cpu_read`, `gic_cpu_write` |

Use the historical `int,tid` per-thread streams. The six event enables are the sole observation difference. Freeze both attempts before launching either. Preserve argv, raw streams and QMP snapshot. Identify each vCPU through its Linux TID and QMP attribution. Retain line numbers for the following observations.

1. CPU1’s distributor write at offset 0xf00, size 4, value 0x18001 establishes entry to the model’s SGIR handler. The pinned source sets the target pending state and sender bit, then calls GIC update. There is no tracepoint on the pending bit store itself.
2. In the CPU1 stream, look for the target CPU0’s IRQ 1 best-candidate event immediately after the opening write, followed immediately by the CPU0 IRQ level-1 decision. The level trace precedes the call that applies the modeled output. Report the exact records seen. A missing record leaves the corresponding stage unobserved. The trace lacks an invocation ID. Adjacent records are evidence of a same-thread sequence and cannot uniquely label an untraced intervening model invocation.
3. Keep subsequent observations on CPU0 separate. Record its architectural IRQ entry, the GIC acknowledge event, the completed IAR read at offset 0x0c (INTID 1, source 1, value 0x401), and the matching EOIR write at offset 0x10. Bind to the guest’s original trap and EOI rows where available. CPU0 may send the reverse SGI to CPU1. Record that direction separately. An absent CPU0 exception after an observed line decision leaves architectural delivery unobserved.
4. Report CPU0’s existing sparse checkpoint 65536, lease, same-CPU relative order with its trap when available, and the marker/error on partial captures. The observer performs no additional guest MMIO operations. Relative order across CPU0’s checkpoint and CPU1’s SGIR, elapsed cross-CPU latency, exact pending-store time, physical timing and migration qualification all remain open. Wall-clock elapsed and host trace overhead are descriptive; a single pair cannot measure repeatable timing impact.

## Dependency for a checkpoint/SGIR ordering result

The local 11.1.3 configure recipe passes `--disable-plugins`. The original build directory has `#undef CONFIG_PLUGIN` and `CONFIG_TRACE_LOG` in `build/config-host.h`. The stripped shipping executable and unstripped build have the same GNU build ID. The six log events are compiled into the executable. Implementation references in QEMU are `hw/intc/arm_gic.c` (`gic_dist_write[l]`, `gic_update_internal`, `gic_acknowledge_irq`, `gic_cpu_read`, `gic_cpu_write`), `hw/intc/trace-events`, `util/log.c` and the generated `build/trace/trace-hw_intc.h`.

To pursue the full ordering gate, use a separate build directory, the matching 11.1.3 source archive (SHA-256 `5a8289c7940dc77d3873c81a6c29ba3afe1446fc9d36e77fa1658cdbecb84b22`), the same ARM-Speed patches, target/TCG configuration and toolchain. Change the plugin setting to `--enable-plugins`, build the AArch64 system emulator with one job, and record its distinct digest and build ID. The public header `include/plugins/qemu-plugin.h` supplies translation-time instruction selection, execution/memory callbacks and register access. Freeze the same images and compare plugin-off and narrowly instrumented runs on that same rebuilt model separately from the stock-model pair. Any future observer must bracket real side effects, identify the CPU0 M checkpoint by its actual task/lease/index and completed journal publication, and join positive host release/acquire markers without assuming cross-vCPU clock order. Merely reaching an instruction before execution does not establish completion of that instruction. This step has no implementation or live observation in this stage.

## 한국어

기존 MG2 첫 6회는 0/6이다. 1단계의 조기 종료 다섯 회에서 CPU1은 SGIR 값 0x18001을 기록했고, 클러스터 첫 회에는 M이 마지막 체크포인트 뒤에 CPU0으로 복귀했다. 이번 진단은 동일 클러스터 이미지 한 쌍에서 모델 내부의 새로운 사건을 확인한다. 먼저 GIC 추적을 끈 대조군, 다음으로 표의 기존 이벤트 여섯 개를 켠 관측군을 각각 처음 한 번 실행한다. 트리거, 산술, 루프 경계, 온라인 CPU, 예외 관측기 및 시간 한도는 동일하다. 두 시도의 실패도 그대로 보존한다.

CPU1 스레드에서 SGIR 쓰기 진입, CPU0 대상의 IRQ 1 최우선 후보, 이어지는 IRQ 출력 1 결정의 실제 행을 보존한다. pending 비트 쓰기 자체에는 이벤트가 없고 출력 이벤트는 모델의 출력 적용 전이다. 연속된 행은 같은 스레드 관측이며 별도 호출 ID는 없다. CPU0에서는 실제 IRQ 진입, acknowledge, 반환된 IAR 0x401 및 같은 값을 쓴 EOIR를 따로 확인한다. 기존 게스트 트랩·EOI 행과 결합하고 역방향 SGI는 별도로 표시한다. CPU0의 최종 체크포인트와 같은 CPU의 트랩 순서는 확인 가능하지만 CPU1의 쓰기와 체크포인트 사이의 순서는 결정할 수 없다. 한 쌍에서 실물 지연이나 이주 성공률을 추정하지 않는다.

제공된 QEMU 빌드는 플러그인이 꺼져 있다. 전체 순서 관측에는 위에 고정한 소스와 패치, 동일 도구·설정을 사용한 별도 플러그인 활성 QEMU와 단일 작업 빌드가 필요하다. 재빌드한 모델 안에서 플러그인 비활성·활성 대조를 새로 고정한 뒤 실제 체크포인트 커밋과 송신의 양의 표식으로 인과 간선을 계산해야 한다. 이 단계에서는 해당 빌드나 모델 실행을 수행하지 않는다.

## Collector and individual verdicts

Use a new native Linux directory, the pinned executable, Python and pyelftools. Freeze the pair before either launch. Supply the freeze SHA printed by the first command and retain each session SHA printed by the two runs. Run each position in its assigned slot after preserving its predecessor.

```sh
python3 tools/pi4_mg2_gic.py freeze NEW_DIR PINNED_QEMU
python3 tools/pi4_mg2_gic.py run NEW_DIR FREEZE_SHA256 1
python3 tools/pi4_mg2_gic.py replay NEW_DIR FREEZE_SHA256 1 CONTROL_SESSION_SHA256
python3 tools/pi4_mg2_gic.py run NEW_DIR FREEZE_SHA256 2
python3 tools/pi4_mg2_gic.py replay NEW_DIR FREEZE_SHA256 2 TRACED_SESSION_SHA256
```

Replay records line numbers within each TID, committed guest row indices and marker-level matches. A null value records a missing or ambiguous join. Replay exit 2 preserves partial evidence, including a traced case without a unique opening write. Exit 0 concerns capture completeness. The actual SGI rows, receiver chain and guest terminal determine the observation. Store replay output outside the immutable session directories. [Collector](../../tools/pi4_mg2_gic.py) · [Host parser checks](../../tools/test_pi4_mg2_gic.py)

Linux에서 새 디렉터리에 두 순번을 먼저 고정하고 지정된 슬롯에서 한 번에 한 게스트만 실행한다. 각 세션의 SHA로 위 명령을 재생한다. null은 누락되었거나 단일 사건으로 결합할 수 없는 관측이다. 코드 2는 불완전한 근거를 보존한다. 코드 0은 수집 범위에 대한 결과이며, 기존 이주 기준은 별도다.
