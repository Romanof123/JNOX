# Completion supervision in an ordinary native package / 일반 보호 채널의 완료 감시

REQ-019/024 · CHG-312 · IPC-010 · IMP-312 · VER-019/024 · EVD-312. A fresh verifier examined 14 original serial boots on single-CPU Cortex-A72 QEMU virt using FIFO and RR, the normal TCG clock and a 62.5 MHz guest counter. Twelve boots ran the ordinary watchdog and two ran the rebuilt qualification worker. Four additional cold boots tested the unmonitored default and an unchanged historical ELF. [Acceptance #402](https://github.com/Romanof123/JNOX/issues/402) · [Source and build instructions](https://github.com/Romanof123/JNOX/blob/fc3bfd6830e284a1bf925e2595a56932e6ebcd42/examples/native-safety-watchdog/README.md).

| Operating case / 일반 실행 | FIFO and RR observations / FIFO·RR 실측 |
| --- | --- |
| Healthy / 정상 | 29 protected completions per boot. Maximum gap 2.759056 ms FIFO and 2.762688 ms RR. Two post-COMPLETE observations in each boot, phases 0 and 1, were already over 1 ms from target. |
| Controller sleeps or computes / 절전·계산 지연 | With the output high, all four deadlines expired on physical timer interrupt 30. The interval from the first disallowed tick to the output-low readback was 67.792–136.528 µs. |
| Controller exit / 제어기 종료 | Both exits inhibited the output before the next watchdog deadline. |
| Generation recovery / 새 세대 복구 | Four further boots timed out and rejected late COMPLETE. Both scheduled generation-2 pairs, 625,000 ticks apart, permitted explicit rearm. Measured pairs differed from exact spacing by +26 FIFO and −4,923 RR ticks and both stayed inhibited. |
| Qualification ABI / 기존 검증 ABI | Two rebuilt workers used fused SVC63. Two additional boots used the byte-identical historical worker `be0d42e614cb76aa21733f0726a63a4aedab1d23fa4fcfe971b8d54e9f3aba07` on the new kernel. All four recorded 29 protected completions and an output rise and fall. |
| Default channel / 기본 채널 | Both additional unmonitored boots recorded the expected rise and fall, role exits 71/72/73 and zero watchdog events. |

The independent reducer joined protected UART records to guest RAM, read the terminal output register, walked per-process page tables and checked linked worker instructions. It recovered 252 actual heartbeats and rejected 11 altered records. All eight induced ordinary timeouts inhibited output and refused late COMPLETE with −110. The ordinary worker had no unresolved syscall sites and used standard protected SVC60/61 without qualification calls 62/63. All 14 protected journals recorded zero loss. The ordinary scheduler journal stored only an initial sequence of events. Its captures establish IRQ30 as timeout source but do not identify the comparator value or interrupted instruction at expiry. Four packager and three direct kernel build configurations were refused as designed. Four host watchdog arithmetic tests passed.

일반 패키지에서 5 ms 작업의 두 위상을 2.5 ms 간격으로 실행했다. 정상 FIFO·RR에서 각각 29회 완료했고, 완료 간격 최댓값은 2.759056 ms와 2.762688 ms였다. 두 정책 모두 첫 두 위상에서는 완료 후 상태를 읽은 시점에 이미 목표부터 1 ms를 넘겼다. 지연·복구 시나리오에서 실제 감시 기한을 넘긴 8회 모두 인터럽트 30이 켜져 있던 가상 출력을 차단했고 늦은 완료는 거부됐다. 제어기 자체가 종료된 경우와 새 세대의 명시적 재무장도 별도로 관찰했다.

독립 검증팀이 최초 14회의 UART와 RAM을 대조했다. 감시 없는 기본 패키지와 기존 ELF를 그대로 올린 호환성 대조 4회는 별도로 수행했다. 이 결과는 해당 가상 CPU에서의 유한 실행 기록이다. 전체 위상 1 ms, 다른 경계 작업에서의 5 ms 완료 간격, 실물 시간 상한 및 제품 승인은 각각 열려 있다. [Timing admission / 시간 수용 #389](https://github.com/Romanof123/JNOX/issues/389) · [Expiry and arrival / 만료·도착 #401](https://github.com/Romanof123/JNOX/issues/401).

## Reproduce the readback / 원본 재판독

The local frozen revision `98163cbef9d436e1216eb53c3f86faaf23d6e2aa` and the [published source](https://github.com/Romanof123/JNOX/commit/fc3bfd6830e284a1bf925e2595a56932e6ebcd42) have the same complete tree `06ea8e65c4b2ca260a7895b202676424f61e1c73`. Download the packets below into one directory.

| Packet / 자료 | SHA-256 |
| --- | --- |
| [FIFO raw captures](native-permit-ordinary-watchdog-fifo.tar.xz) | `b8d107e0f509954f194d0a15bd5edd0009c15159540aa8bb88a56d44f62ef4a9` |
| [RR raw captures](native-permit-ordinary-watchdog-rr.tar.xz) | `fe57f1419c2106fd3a69058bdebe40f09a882f09063fd22b6497e4e7b5635e7f` |
| [Independent bilingual report, reducer, original package images and historical controls](native-permit-ordinary-watchdog-replay.tar.xz) | `1bde6b1078473a3130c10a9c5087bd3e66aceb68f6ab19fec52188a7cda6b464` |
| [Unmonitored default controls](native-permit-watchdog-default-controls.tar.xz) | `0f4a73d913bf23ae1f6e04054ad45f415819e48580921a76c6abab8ef46bcd1e` |

Linux with Python 3, GNU nm and AArch64 GNU objdump can replay the observations offline. The independently derived `boots.json` and `mutations.json` were byte-identical after detached extraction. / 아래 절차는 새로 추출한 자료에서 판정 JSON의 바이트 일치까지 검사한다.

```sh
mkdir packet default-control
for part in fifo rr replay; do
  tar -xf "native-permit-ordinary-watchdog-$part.tar.xz" -C packet
done
bash packet/scripts/replay.sh packet first-results
tar -xf native-permit-watchdog-default-controls.tar.xz -C default-control
python3 default-control/baseline-check.py default-control
```
