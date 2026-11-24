# PR 237 independent first boots / 독립 최초 부팅

Public branch kernel/amc-idle-carry-in at 0cc2e63c1dc5ba10b4103237846b180c295417cb matched the fetch. Prospective plan 70a0aee precedes the implementation. Four first builds and cold boots passed with JNOX_BOARD=virt JNOX_PROFILE=amc JNOX_APP=<case>. One Cortex-A72 under QEMU TCG, GICv2 physical timer, one build job and serial execution. All session and freeze digests sit outside their respective directories.

공개 브랜치의 정확한 커밋에서 네 경우 모두 첫 빌드와 첫 부팅이 통과했다. 기존 세 경우는 변경 전 판정기와 현 판정기로 재검증했다. 호스트 테스트 138개가 통과했으며 유휴 길이 1~4, LO와 생존 HI 수요 각각 1~2의 16조합을 포함한다. 게스트에서 측정한 조합은 (1,2,2) 하나다.

| Case / 경우 | Observed service or transition / 관측 결과 |
|---|---|
| Exact LO | 0,1,0. LO maintained. |
| Exact HI | 0,1,1,1,3,0. b2 to first HI payload 145883 ticks. Fresh LO 8/36 and SVC before b6. |
| Over HI | Same owners, fourth HI grant denied. b2 to first HI payload 158978 ticks. Fresh LO 8/36 and SVC before b6. |
| Staggered | b0–6 task/generation 0/0, idle, 0/1, 3/0, 1/0, 1/0, 3/0. |

Staggered origin 640734, quantum 3125000 at 62500000 Hz. At b1 the guest returned to its linked private WFI frame. Its physical timer IRQ 30 read back compare 6890734, with interrupted PC four bytes past idle entry and IRQ sample 6987228. Arm to IRQ took 3052591 ticks (48.841456 ms). Compare to IRQ took 96494 (1.543904 ms). The b2, b3 and b4 releases had absolute deadlines 6, 8 and 8. At b5 LO 0/1 lost its remaining grant and froze at 569840 samples / 2564280. HI 1/0 advanced to 1254477 / 5645146. The first HI payload followed b5 by 152362 ticks (2.437792 ms). Saved HI 3/0 resumed its own frame from 8 / 36 to 16 / 72. Its b3 release to SVC entry took 9637010 ticks (154.192160 ms), leaving 5987990 ticks before b8. Release-to-retirement samples for 0/0, 1/0 and 3/0 were 3283440, 6374575 and 12620115 ticks. Same-process exception order matched external replay.

유휴 프레임 복귀 및 물리 타이머 IRQ를 게스트와 같은 QEMU 프로세스의 예외 로그로 대조했다. b5에서 작업 한 단위를 수행한 LO가 폐기됐고 두 HI 작업은 각자의 진행 상태를 보존했다. 생존 HI의 실제 SVC 완료는 예정 릴리스에서 154.192160 ms 후였다. 수치는 해당 가상 머신의 단일 실행 관측치이며 실제 보드 최대 응답 시간과 제품 승인은 별도 근거가 필요하다.

## Separate documentation repair / 별도 문서 수정 필요

The plan directs the caller to set JNOX_AMC_APP=stagger-idle. The build script reads JNOX_APP and sets JNOX_AMC_APP internally. The documented selector alone exited 101. The public input JNOX_APP=stagger-idle succeeded. An unknown app and the Pi 4 board selection each exited 101. Correct the plan in a separate change. No source edits or public writes occurred.

계획서의 빌드 입력을 JNOX_APP=stagger-idle로 정정해야 한다. 문서 표기만 사용한 실행과 미지원 앱·보드 지정은 각각 종료 코드 101을 반환했다.

## Replay / 재검증

Extract the ZIP. With Python 3 and pyelftools installed run sha256sum -c SHA256SUMS then python3 check.py. The checker runs seven offline replays and independently reconstructs release rows, grants, absolute compares, ring sums, idle intervals and finite response from UART. Raw first files, maps, build logs, checker source, digests, execution scripts and tool versions are included.

[Original captures, builds and independent replay](ordinary-amc-stagger-idle-first.zip) · SHA-256 6f8a135388ea0669feaf4288faff4181d373a70494cda35ebc09ee832aadb3ee.
