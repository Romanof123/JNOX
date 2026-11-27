# EVD-339 · Guest compositor recovery / 합성기 복구 시험

Three cold boots of the same AArch64 QEMU virt kernel (SHA-256 `75801b34ca48e5f2c7bd30800b73ec213c6a70c5053e586e64ea43cbb0e1dacf`, source `b07214dfb5f2744448c543ab535760df7a0fcb20`) ran a primary compositor, persistent client, separate standby, broker and 10 ms controller. The controller had no display or socket grant. Each run retained all 4,096 absolute deadlines and completion times. [Predeclared criterion](https://github.com/Romanof123/JNOX/issues/449#issuecomment-7208017547) · [Independent replay results](summary.json)

| Scenario | Outcome | Maximum gap during recovery | Whole-run maximum gap | Releases late by at least one period |
| --- | --- | ---: | ---: | ---: |
| Primary fault, clean successor exit | Same client reconnects, first successor frame and two subsequent key updates observed | 10.137904 ms | 49.418464 ms | 3 |
| Normal primary exit | Dormant spare dismissed, both identities retired at the broker, broker exits normally | — | 10.242352 ms | 0 |
| Primary and successor fault | Second fault exhausts replacement, display authority revoked, controller keeps running | 10.599056 ms | 468.369296 ms | 45 |
| Separate headless boot | Same controller ELF, sole process, no framebuffer | — | 10.296560 ms | 0 |

For the two fault runs the first independently confirmed successor frame fell within guest-time upper bounds of 130.097024 ms and 129.978944 ms respectively. The actual presentation could have occurred earlier within each bracket. The two whole-run gaps above 20 ms occurred before the first confirmed primary frame. Their cause is open. The ordinary AF_UNIX two-window regression also passed. Original fault images, wire journal, page tables, control clock samples and exits supported the independent replays. [Startup timing follow-up #453](https://github.com/Romanof123/JNOX/issues/453)

Download the [original evidence archive](jnox449-evidence.zip) and [portable review kit](jnox449-review-kit.zip). A fresh restore verified 394 files and all 30 decoded 128 MiB RAM images. The reader reproduced the three corrected scenarios and six rejected corruptions. All 16,384 timing samples matched restored memory. Checksums and reproduction commands are in the [replay instructions](REPLAY.md).

The initial kernel admitted an IPC identity for the dormant spare but dismissed only its scheduler slot on clean primary exit. The broker waited indefinitely. The corrected kernel retires that identity through normal IPC teardown. The first offline observer also assumed that successor device initialization preceded its first restored instruction. Raw CPU state established the reverse order. Separate deliberate alterations of a frame pixel, fault owner and timing witness were rejected in both corrected fault runs.

동일 가상 커널에서 합성기 고장과 재연결, 주 합성기 정상 종료, 예비 합성기까지 고장 난 경우를 확인했다. 화면 복구 두 구간에서 독립 제어 앱은 20 ms 이내에 계속 완료됐다. 전체 부팅에서 나온 두 큰 간격은 각각 네 번째와 열 번째 인덱스에서 관측됐다. 제어 앱 단독 부팅과 정상 종료에서도 4,096개 시각 전체를 확인했다. 이 결과는 단일 CPU 가상 보드에서 관측한 값이다.
