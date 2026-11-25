# Independent CPU0 normal-ELF weighted verification / CPU0 일반 ELF 가중 스케줄링 독립 검증

Public implementation 100d32e1b6e49277c9801723b8cc5b0dd3ad801f. QEMU virt Cortex-A72, one CPU, 62.5 MHz. The same 625000-tick quantum, H=781250 and G=156250 applied to both input populations. The extended cohort changes only work volume; every executable uses the ordinary loader, independent writable image and production scheduling path. Input manifest 9892ad7392bd21ce87c71e96fbb91db9405a820ff9c476dfd9029ace92e863a7.

## Sealed originals / 원본 최초 관측

Originals are preserved in jnox297-original-firsts.tar.gz, SHA-256 605ce7d98e4afeb9edb0d8f0a4d58c4c95673c2fa18e282bdd9f752e334bc4b3. Original default share reached 25/36, swap 13/36, triple 49/60. No scored prefix violated an arithmetic or grant bound during that common-ready span. Original wake/default produced three actual resumed user clock reads within bounds; peer service during its waits was 2474346, 1711854 and 0. The last two missed 1875000 ticks. Wake/icount passed all three. Original share/icount recorded 36/36 qualified window with 7500120/15000240 service and correct private products. The 90-second watchdog ended UART collection at 1149 complete rows and cut the following row; loss counter and final exits were unavailable on that boot. Original scores retain that limit. The ordinary solo, all-sleep, simultaneous-due and final yield cases completed. Startup, cooperative, FIFO/RR and policy numeric and configuration controls are recorded in the original capsule.

최초 기본 시계에서는 CPU 계산 하나가 일찍 끝나 1:2, 뒤바꾼 2:1, 1:2:3의 필요 관측 길이에 미달했다. 세 번의 기본 시계 대기 중 두 번째와 세 번째에는 상대 앱의 서비스가 부족했다. 원본 icount의 1:2 최초 36회와 사설 메모리 결과는 확인됐지만, 출력 감시 시간이 끝나 최초 실행의 최종 출력은 잘렸다. 이 원본 판정은 그대로 보존한다.

## Longer inputs, first default boots / 확장 입력 기본 시계 최초 부팅

| Package | Frozen window | Physical service by slot, ticks | Productive interrupt PCs by slot | Final status |
| --- | --- | --- | --- | --- |
| CRC 1, histogram 2 | 36/36 | 7563657 / 15081733 | 12 / 24 | 237 / 70 |
| CRC 2, histogram 1 | 36/36 | 15111026 / 7542390 | 24 / 12 | 237 / 70 |
| CRC 1, histogram 2, energy 3 | 60/60 | 6317305 / 12579009 / 18882070 | 10 / 20 / 30 | 237 / 70 / 179 |
| CRC 1, timed reporter 2 | three 40 ms waits | peer 2466269 / 2500316 / 2501756 during waits | three wake IRQs in CRC loop | 237 / 7 |

Full progress counters were 134217728, 536870912 and 1073741824. Private RAM matched CRC 2124873618, histogram [2097152,16777216,58720256,117440512,146800640,117440512,58720256,16777216,2097152], energy 5864240971776 and reporter mask 7. Across 211, 107, 410 and 13 complete rows respectively, minimum selection, rational service/floor and next compares replayed exactly with zero dropped rows. All registered H/G and share inequalities passed in their windows. The reporter received zero blocked charge, restored at 0x40019c, and called the user clock SVC at 0x400168. All three private absolute deadlines and post-wake clock results agree with the captured counter samples under one inferred per-boot epoch. Actual EL0 return falls between the restore-hook timestamp and following clock syscall. The conservative endpoint inequalities pass. No instruction-cycle-exact ERET timestamp is claimed.

확장 입력의 첫 부팅에서 1:2와 2:1은 각각 36회, 1:2:3은 60회 연속 경쟁 구간의 기준을 충족했다. 타이머가 각 앱의 실제 계산 함수를 중단한 횟수와 물리 서비스량은 위 표에 있다. 세 번의 대기 모두 상대 앱이 3Q 이상 계산했고, 기자 앱은 막혀 있는 동안 0틱을 정산받았다. 복귀한 사용자 코드가 기록한 시각, 전체 사설 계산 결과, 종료 상태를 검증했다.

## Supplemental terminal capture / 원본 ELF 추가 종료 관측

After all four extended boots, the identical original share kernel (546e3ece41c17f042dd3730d51196ea504aa236afac65718a547c7162be07659) ran at shift=3 again. An initial diagnostic capture stopped when the HMP parser rejected its output filename. The preserved retry used QMP's typed pmemsave. While the kernel was printing its completed journal, it captured terminal EXIT_STATUS 52/197, all 1616 journal rows, zero drops, and both full private products. Replay from those physical bytes passed. The first-36 service matched the original exactly. The UART file and independently rendered RAM view are separate.

추가 icount 실행은 첫 시도의 콘솔 출력 누락을 보완한다. 첫 진단의 모니터 명령 실패도 보관했다. 재관측에서 RAM 원본으로 1616행과 종료 상태 52/197을 확인했다. 이 성공은 최초 실행의 누락을 대체하지 않는다.

These observations qualify the specified finite QEMU cases. Physical-board timing, USB, integrated-load tolerance and historical #184 remain open; original #177, #178 and #183 outcomes remain unchanged. No dedicated guest overflow/stress case was scored here. 실물 보드 시간 특성, USB 및 통합 부하 성능은 미검증이다. 최초 실패와 기존 이슈 결과는 그대로 남는다.

## Download and replay / 원본 내려받기

Download the four original parts to one directory, then run the following Python code there. It verifies the original file before writing it. 원본 조각 네 개를 같은 디렉터리에 저장한 뒤 아래 코드로 합치면 검증팀이 봉인한 최초 압축파일을 복원한다.

- [native-weighted-applications-original.tar.gz.part01](./native-weighted-applications-original.tar.gz.part01) · SHA-256 `32b89993b529a7ad2390b606021a60e33f295d77d280271715d2f23919a3b163`
- [native-weighted-applications-original.tar.gz.part02](./native-weighted-applications-original.tar.gz.part02) · SHA-256 `57baac361b3c58e3e71e6ad850bab4722c5b9a9ea8ecd68ba36d1adc00df027e`
- [native-weighted-applications-original.tar.gz.part03](./native-weighted-applications-original.tar.gz.part03) · SHA-256 `da3f491bb8f0fffa926df5b2e1799d0c579730a2ad2c948354955f5caed51ee5`
- [native-weighted-applications-original.tar.gz.part04](./native-weighted-applications-original.tar.gz.part04) · SHA-256 `abd52cc589c924985a12360d08f535b71ef151745ab7556cffa979ee821c753e`

~~~python
from pathlib import Path
from hashlib import sha256

name = "native-weighted-applications-original.tar.gz"
data = b"".join(Path(f"{name}.part{i:02d}").read_bytes() for i in range(1, 5))
assert sha256(data).hexdigest() == "605ce7d98e4afeb9edb0d8f0a4d58c4c95673c2fa18e282bdd9f752e334bc4b3"
with Path(name).open("xb") as archive:
    archive.write(data)
~~~

[Extended application and terminal captures / 확장 실행과 종료 원본](./native-weighted-applications-extended.tar.gz) · SHA-256 `fd66960737db6037bdc80f0a420b58e629f245cda1293f2fea3550a63aaa82cb`.
