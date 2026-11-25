# IPC-010 · Expiry-latch recovery / 만료 래치 복구

Six original cold single-core Cortex-A72 QEMU virt boots used the ordinary three-process FIFO and RR profiles. PID 0/1/2 exited 71/72/73. / 일반 FIFO·RR 구성으로 단일 가상 Cortex-A72의 최초 여섯 부팅을 기록했다. 송신자·제어기·감독자의 종료 코드는 71·72·73이었다.

Built commit e2328a76ead53df8cc34287cab74050265ce08f6 and [public revision](https://github.com/Romanof123/JNOX/commit/088d94a412a9420723a6a98e7918ccc1751ba396) share tree 5091b7de28aa76f573de3027c95ec9adc87ec32a. Replay reconstructs the recursive tree, hashes both commit objects and verifies archived build inputs. / 재검증기는 전체 트리와 두 커밋 객체, 보관한 빌드 입력의 해시를 확인한다.

Raw monotonic counter ticks, 62,500 ticks/ms. Response and completed-check gaps give the minimum lower and maximum upper bounds across the boot. Device transitions use register swap/readback brackets. / 단위는 틱이며 62,500틱이 1 ms다. 응답과 완료 간격은 관측 구간의 최소 하한과 최대 상한이다. 장치 전이는 레지스터 교환·재판독 전후의 틱을 사용한다.

| Boot / 부팅 | Checks / 판정 | Release to completion / 응답 | Completion gap / 완료 간격 | Safety / scheduler drops |
|---|---:|---:|---:|---:|
| 01-fifo-recover-latch | 27/27 | [9469, 167276] | [161175, 341065] | 0 / 314 |
| 02-fifo-recover-hold | 25/25 | [9087, 170126] | [157895, 323915] | 0 / 292 |
| 03-fifo-restart | 17/17 | [11358, 170649] | [156330, 324370] | 0 / 373 |
| 04-rr-recover-latch | 27/27 | [11248, 187318] | [138350, 338280] | 0 / 315 |
| 05-rr-recover-hold | 25/25 | [9212, 165184] | [165495, 322066] | 0 / 295 |
| 06-rr-restart | 17/17 | [12728, 157580] | [171586, 318236] | 0 / 373 |

| Boot | Gen1 seq3 issue | Expiry decision | Device fall | OPEN2 | Early REARM | Late REARM | Device rise | Withdrawal fall |
|---|---:|---:|---|---:|---:|---:|---|---|
| 01-fifo-recover-latch | 5038693 | 6356767 | [6358521, 6358529] | 6923387 | 7550254 | 8171378 | [8234975, 8234979] | [8568380, 8568386] |
| 02-fifo-recover-hold | 5040685 | 6357238 | [6359320, 6359325] | 6924542 | 7549967 | — | — | — |
| 04-rr-recover-latch | 5060202 | 6380045 | [6381673, 6381677] | 6943164 | 7572010 | 8190593 | [8257537, 8257542] | [8589671, 8589677] |
| 05-rr-recover-hold | 4986285 | 6306319 | [6308468, 6308472] | 6870782 | 7496795 | — | — | — |

Both policies established high output at gen1 sequence 3. After silence the kernel latched age expiry, then the device fell before supervisor OPEN2. OPEN cleared sequence, issue, streak, fault, enabled and pending state. Two gen2 set permits were issued exactly 10 ms apart. The early REARM at sequence 1 returned EAGAIN. Sequence 2 committed low. In the positive case the controller consumed a fresh later command and the device rose, then fell on sequence 3 withdrawal. The omission held low through the final observation. Ordinary restart controls withdrew before generation change. The three roles stayed live. / 두 정책 모두 첫 세대 3번 허가에서 출력이 켜졌다. 무송신 후 실제 만료와 하강이 먼저 관측됐고 감독자의 OPEN2가 앞 세대 상태를 지웠다. 둘째 세대 허가 간격은 정확히 10 ms였다. 첫 허가 뒤 재무장은 EAGAIN이었으며 두 번째 허가 후에도 출력은 꺼져 있었다. 양성 시험은 새 명령을 받은 제어기가 신선할 때 출력을 켠 뒤 3번 허가 철회로 껐다. 생략 대조군은 마지막 관측까지 꺼져 있었다. 기존 restart 대조군은 세대 전환 전에 철회했다. 세 프로세스는 전환 중 살아 있었다.

Completed-check gaps exceed the 312,500-tick (5 ms) target. The functional results cover virtual generation recovery. Timing admission, process relaunch, physical response and product approval remain open. / 완료 간격이 312,500틱(5 ms) 목표를 초과했다. 기능 결과는 가상 장치 세대 복구를 다룬다. 타이밍 승인, 종료된 프로세스 재기동, 물리 반응 및 제품 승인은 별도 검증이 필요하다.

Extract and run sha256sum -c SHA256SUMS and python3 scripts/replay.py . /tmp/ipc399-results with GNU aarch64-linux-gnu-nm available. Compare the generated directory to results/. Full UART, RAM, ELFs, manifests, receipts, private snapshots, protected journals and negative replay controls are included. / 압축 해제 뒤 위 명령으로 원시 증거와 판정을 재검증한다. 생성 디렉터리를 보관된 results와 비교한다.

[Raw captures and offline replay](native-permit-latched-recovery.tar.xz) (SHA-256 `86fd79c4c251e0faa004a241d3c4dd9825c0b6698c3d62cb97939fa772ac7912`).
