# Pi 4 directed wake on two cold model boots

[한국어](pi4-smp-first.md) · [Machine-readable result](pi4-smp-first.json) · [Evidence archive](pi4-smp-first.zip)

Both first launches of the same raw image completed two transactions. CPU0 produced sums of 136 and 392. CPU1 woke its higher-priority blocked receiver once per generation, consumed each sum once, and returned 272 and 784. Each CPU1 IRQ interrupted background PC 0x82698 with SP 606144. Each receiver SVC returned control to that same PC and SP. All three task stack guards were intact. The bounded journals committed 10 CPU0 rows and 28 CPU1 rows with zero loss. [Tested source](https://github.com/Romanof123/JNOX/tree/d537835a858bb340c3a8d8507fb7bd5ec3ad20c4)

| Boot / generation | CPU0 samples before send | CPU1 interrupted → resumed count | CPU0 send-call ticks |
|---|---|---|---:|
| 1 / 1 | 960 → 16384 | 27024 → 27056 | 4977 |
| 1 / 2 | 28352 → 28576 | 29680 → 29712 | 2263 |
| 2 / 1 | 400 → 15520 | 26496 → 26528 | 5475 |
| 2 / 2 | 27440 → 27792 | 29184 → 29216 | 2173 |

CPU0 entered the producer at 0x827e0. CPU1 entered the receiver at 0x82c90 and first dispatched background at 0x8261c. Each generation read IAR 1 with source 0, claimed the blocked receiver, completed full-IAR EOI and dispatched the saved receiver continuation at 0x82d78. The 832-byte background and receiver frames started at 605312 and 621728 with SP 606144 and 622560 respectively. Linked Rust probes record actual task execution and per-core MPIDR. Separate QEMU thread logs record the exception/return PCs. Each thread-ID filename joins to a captured Linux task name identifying its vCPU. The mapping files give one-based original line ranges. CPUs 2 and 3 had named TCG threads and no recorded task exception/return events.

The independent protocol was frozen before implementation inspection. The first locked release build succeeded with four warnings using Rust toolchain 1.95.0, the AArch64 bare-metal target and the dedicated Pi 4 SMP selector. Each launch used a fresh four-core 2 GiB raspi4b TCG process, a 30-second ceiling and at least one second of terminal serial quiet. The host then terminated QEMU. Elapsed host times were 1.157 and 1.139 seconds. Both CPUs reported 62.5 MHz. The send-call intervals bracket software operations. Bus delivery time, cross-core latency and a shared counter epoch remain unmeasured. The offline replay rejected wrong CPU, wrong generation, wrong IAR source and a missing IRQ row in each capture.

The archive includes the original UART and per-thread observer bytes, acquisition script, frozen protocol, linked ELF, raw image, map, disassembly, firmware package inputs, build output, manifests, parser and controls. SHA-256 entries in prefreeze.sha256 identify the executable, source tar and package files. path-substitutions.json records original and public hashes for host-path edits in the build log, freezer manifest and two exit records. Identical ELF, image and UART copies are stored once. To reconstruct both replay directories, extract the ZIP, make pyelftools available to Python 3 and run `python reconstruct.py`. Direct raw-kernel loading bypassed the archived boot firmware. Physical startup, hardware timing, energy, interference, WCET and production allocation remain open.
