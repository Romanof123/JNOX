# Controller wake timing during display startup / 화면 시작 중 제어 작업의 기상 시각

The four retained AArch64 guest runs contain 16,384 wake timestamps. Two adjacent intervals exceeded 20 ms, both during startup and before the first independently confirmed primary frame. The largest interval was 468.369296 ms. At its end the controller read its clock 458.516448 ms after that sample's scheduled target. The timestamps establish when the guest observed the delay. They leave the cause open. [Startup timing #453](https://github.com/Romanof123/JNOX/issues/453) · [Compositor recovery #449](https://github.com/Romanof123/JNOX/issues/449)

## Measurement

The control process advances its absolute target by 10 ms each iteration, calls `clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, ...)`, then records the target and its first post-wait clock reading. The 4,096 records per run give 4,095 adjacent wake intervals. Row zero has no previous recorded wake. A wake at least 10 ms after its target increments the controller's `missed` counter and can occur during catch-up after a single long interval. The measurement ends at the clock read. [Control source](https://github.com/Romanof123/JNOX/blob/a18d35f85257cd25d7cdaa3029f4fc8ad542b3a3/examples/native-wayland-restart/control.c) · [Clock calls](https://github.com/Romanof123/JNOX/blob/a18d35f85257cd25d7cdaa3029f4fc8ad542b3a3/examples/native-edf/clock.h)

| Scenario | 99th percentile adjacent gap, ms | Maximum gap, ms | Gap >20 ms, count | 99th percentile wake lateness, ms | Maximum wake lateness, ms | Wakes ≥10 ms late, zero-based indices |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Primary fault, clean successor exit (`restart-quit`) | 10.074128 | 49.418464 | 1 | 0.208320 | 39.478288 | 3 (4–6) |
| Normal primary exit (`primary-exit`) | 10.067168 | 10.242352 | 0 | 0.189264 | 0.334768 | 0 |
| Primary and successor fault (`spare-fault`) | 10.065584 | 468.369296 | 1 | 58.846144 | 458.516448 | 45 (10–54) |
| Headless, controller alone (`headless`) | 10.051520 | 10.296560 | 0 | 0.155744 | 0.373456 | 0 |

Times below are exact integer nanoseconds from the guest monotonic clock. The two rows enumerate every whole-run gap above 20 ms. Percentiles above use nearest rank independently for the two distributions.

| Scenario / sample | Previous wake | Current absolute target | Current wake | Wake − previous wake | Wake − target |
| --- | ---: | ---: | ---: | ---: | ---: |
| `restart-quit` / 4 | 90,777,568 | 100,717,744 | 140,196,032 | 49,418,464 | 39,478,288 |
| `spare-fault` / 10 | 151,514,544 | 161,367,392 | 619,883,840 | 468,369,296 | 458,516,448 |

The 20 ms acceptance bound was specified for the recovery window. Inside that window the historical maxima were 10.137904 ms and 10.599056 ms, respectively. #453 separately tracks the early whole-run outliers. Root-cause work needs early timer arm/IRQ/service, running process and host capture/VM-stop observations on a common timeline. These four wake arrays alone do not distinguish guest scheduling from host/emulator stalls. [Frozen window and whole-run results](https://github.com/Romanof123/JNOX/blob/a18d35f85257cd25d7cdaa3029f4fc8ad542b3a3/docs/evidence/native-wayland-recovery/summary.json)

## Reproduction

EVD-339 recorded source `b07214dfb5f2744448c543ab535760df7a0fcb20` on QEMU 6.2.0, AArch64 `virt,gic-version=2`, Cortex-A72, one vCPU, ordinary TCG timing. All four runs use the same control ELF with SHA-256 `b35162105fe5688c60088ec5a7e90a4a311be7d4e2f5d4998f1800f556034a51`. The original independent review matched the timing CSVs to the guest arrays. The command below uses Python's standard library, checks the published archive digest and the individual CSV digests, then recalculates every interval and wake delay. It reads the manifest and four CSV members and starts no guest. [Original verification and archive identity](https://github.com/Romanof123/JNOX/blob/a18d35f85257cd25d7cdaa3029f4fc8ad542b3a3/docs/evidence/native-wayland-recovery/REPLAY.md)

```sh
python3 verification/xwayland/startup-control.py /path/to/jnox449-evidence.zip
```

## 한국어

제어 프로세스는 10 ms씩 증가하는 절대 목표 시각까지 대기한 뒤 단조 시계를 읽었다. 네 부팅에서 목표·기상 시각 16,384쌍을 재계산했고, 바로 앞 기상과 비교할 수 있는 구간은 16,380개였다. 표의 기상 지연은 해당 회차의 목표 시각 대비 차이다.

기동 중 긴 구간은 두 번이었다. 주 합성기만 고장 난 부팅의 4번 표본은 직전 기상과 49.418464 ms 떨어져 있었고, 자신의 목표 시각보다 39.478288 ms 늦었다. 두 합성기가 모두 고장 난 부팅의 10번 표본에서는 각각 468.369296 ms와 458.516448 ms였다. 절대 목표가 매 회차 10 ms씩 증가하므로 한 번 밀린 뒤에도 각각 3회, 45회 연속으로 목표보다 한 주기 이상 늦을 수 있었다. 정상 종료와 화면 없는 부팅에는 20 ms 초과 구간이나 한 주기 이상 늦은 기상이 없었다.

원래 20 ms 수용 기준은 합성기 교체 구간에 적용했다. 두 이상치는 최초 기본 화면을 외부에서 확인하기 전에 발생했고, 교체 구간 자체의 판정은 유지된다. 원인을 판정하려면 초기 타이머·IRQ 처리, 당시 실행 프로세스와 호스트 관측 중 가상 머신 정지 구간을 대조해야 한다. 이 네 부팅의 기록만으로 실보드 지연 상한을 정할 수 없다.
