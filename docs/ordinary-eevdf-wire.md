# POL-012 journal v1 / 저널 v1

`jnox_eevdf_app_journal` is contiguous little-endian u64 data. Numbers below are zero-based word positions within each section. / 아래는 각 구역의 리틀엔디언 u64 워드 위치(0부터)이다.

| Section / 구역 | Fields in order / 순서 |
| --- | --- |
| Header / 헤더 (12) | version 1, state (0 init, 1 active, 2 complete, 3 failed), failure code, CNTFRQ, Q, origin E, committed row count, accepted wakes, reported first responses, finished tasks, stack bytes (16384), row capacity (512) |
| Row / 행 (512 × 33) | 0–6: ordinal, owner, opening ready mask, start, programmed compare, vector stop, cause (1 IRQ, 2 wait SVC #1, 3 finish SVC #2). 7–10: saved frame/ELR, opening restored frame/ELR. 11–14: saved x0–x3. 15–20: IRQ-woken sequence or zero, accepted wake time or zero, closing ready mask, opening V numerator/denominator, raw IAR (zero on SVC). 21–32: in ID order, opening runtime numerator, deadline numerator, weight and remaining physical request per task. |
| First work / 첫 작업 표본 (8) | Task-side physical counter read just after wait returns, indexed by sequence minus one. / 대기 SVC 복귀 직후 태스크 카운터, 순서 k−1. |

Use the committed prefix even after a failure. An interrupted uncommitted row cannot establish a completed lease. `jnox_eevdf_app_stacks` is three consecutive 16 KiB stacks in ID order. Sequence is in x0 for wait; finish uses x0–x3 for ID, count, sum and weighted sum. Other argument words have no specified value. Capture the raw image and same-guest exception trace on the first terminal marker, either `EEVDF APP COMPLETE` or `EEVDF APP FAIL`. / 실패 시에도 완료 행 수만 사용한다. 작성 중이던 행은 완료된 실행권의 근거가 될 수 없다. 스택 심벌은 ID순 16 KiB 스택 셋이다. 대기는 x0에 순번, 완료는 x0~x3에 ID·건수·합·가중합을 전달한다. 그 밖의 인수 값은 정하지 않는다. 최초 성공 또는 실패 종료 마커에서 원시 이미지와 같은 게스트 예외 추적을 보존한다.

The opening restored frame/ELR belong to that row’s owner on dispatch. Its saved frame/ELR describe the same owner's outgoing exception. Initial frames are at stack top minus 832 bytes and enter batch_a, batch_b or responder, with SPSR 0x345. A resumed task restores its last saved frame and ELR. The final finish SVC has no ERET. / 행의 복원 프레임·ELR은 소유자의 디스패치 시점, 저장 프레임·ELR은 같은 소유자의 예외 진입 시점이다. 최초 프레임은 스택 꼭대기 832바이트 아래이며 SPSR은 0x345이다. 재개 시 마지막 저장 프레임과 ELR을 복원하고 마지막 완료 SVC에서는 ERET하지 않는다.

For each wait, the next dispatch start must be strictly earlier than that sequence's due time. This supplies an observable upper bound for admission into the sleeping set. The fifth wait and immediately following snapshot show the single live change of B's weight and request. / 매 대기 뒤 다음 디스패치 시작 시각은 해당 예정 시각보다 빨라야 한다. 이 시작 시각으로 수면 집합 편입의 상한을 확인한다. 다섯째 대기와 바로 다음 상태로 B의 가중치·요청 변경을 확인한다.

Build a fresh linked kernel on Linux or WSL with no benchmark features. Capture the exact source commit, ELF, SHA-256, symbols, disassembly, UART, QEMU version/arguments, same-process exception log and QMP memory dump. The capture command prints the frozen session SHA-256 for replay with pyelftools. / Linux 또는 WSL에서 벤치마크 기능 없이 빌드한다. 소스 커밋·ELF·해시·심벌·디스어셈블리·UART·QEMU 버전/인자·같은 프로세스의 예외 기록과 QMP 메모리 덤프를 보존한다. 캡처가 출력한 세션 SHA-256으로 재구성한다.

```sh
JNOX_BOARD=virt JNOX_PROFILE=eevdf JNOX_APP=telemetry python3 tools/build_kernel.py --arch aarch64
python3 tools/ordinary_eevdf_capture.py NEW_DIRECTORY ELF SOURCE_SHA
python3 tools/ordinary_eevdf_replay.py NEW_DIRECTORY SESSION_SHA256
```

[English contract](ordinary-eevdf-plan.en.md) · [한국어 계약](ordinary-eevdf-plan.md)
