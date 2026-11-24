# #186 second timer-observation cohort / 두 번째 타이머 관측 집단

[First contract / 최초 계약](native-weighted-timer-observation.md) · [#186](https://github.com/Romanof123/JNOX/pull/186) · [Timing investigation / 시간 조사 #184](https://github.com/Romanof123/JNOX/issues/184)

The original plain and instrumented guests and official replays returned success, but their Linux temporary directories were unavailable at transfer. The raw recordings were unrecovered. The original paired timing result remains unqualified. This second cohort has two new first positions, share/default plain followed by share/default instrumented. Run each once on the Linux local filesystem using the same #183 inputs, original QEMU configuration, 20-second watchdog, existing observer and official replay. Only the second position enables `--timer-observation`. Capture and replay remain separate subprocesses in one enclosing WSL invocation. Retain the exact commands, their stdout/stderr/exit status, the capture-emitted session digest, replay output and all available raw files even on failure.

At 62.5 MHz the frozen Q=625,000, H=781,250 and G=156,250 are unchanged. Score every one of 36 timer closures, each scheduler gap, every prefix −H≤2A−B≤2H, total T≥36Q and |A/T−1/3|≤2H/(108Q). The closure bound is Q≤I−R≤H. Keep the tail and both first outcomes. Timer-callback entry, exact vCPU acceptance and timed off-CPU episodes remain unobserved. Declare additional absent or ambiguous records. Preserve host-wall and guest-counter units separately.

Before the first boot, the caller independently provisions a new empty durable directory on Windows D:. Reserve each position on D: before launch. After each position, export a ZIP plus an independent member manifest and archive digest directly there; close and reopen the archive, then use native Windows to reread the actual D: archive and hash every member. Require both verification receipts before starting position two. Archive a failed or partial position, listing absent artifacts and the reason for any unexecuted replay. A transfer/verification failure stops the sequence. An occupied destination cannot be reused. Keep any retained staging data for recovery; never relaunch to manufacture a missing recording.

## 한국어

최초 일반·계측 게스트와 공식 재생은 성공 종료했으나 전송 시점에 리눅스 임시 디렉터리가 사라져 원시 기록을 복구하지 못했다. 최초 쌍의 시간 판정은 미충족 상태로 보존한다. 이번 집단은 share/default 일반, share/default 계측 순서의 독립된 최초 위치 두 개다. 같은 #183 입력과 QEMU, 20초 제한, 기존 관측기·공식 재생기를 사용해 각 위치를 리눅스 로컬 파일시스템에서 한 번만 실행한다. 둘째 위치에만 `--timer-observation`을 준다. 하나의 WSL 실행 안에서 위치마다 캡처와 재생을 수행하고 실제 명령, 각각의 표준 출력·오류·종료 코드, 캡처가 출력한 세션 해시, 재생 결과와 남은 원시 파일을 보존한다.

62.5 MHz에서 Q=625,000, H=781,250, G=156,250을 유지한다. 36개 구간 각각 Q≤I−R≤H, 모든 공백 ≤G, 모든 누적 편차 −H≤2A−B≤2H, 총 서비스 T≥36Q와 |A/T−1/3|≤2H/(108Q)를 그대로 적용한다. 종료 구간과 두 최초 결과를 보존한다. 타이머 콜백 진입, 정확한 vCPU 인터럽트 수락, 개별 오프 CPU 시간은 미관측이다. 추가 누락과 모호성도 기록하고 호스트 시계와 게스트 카운터의 단위를 구분한다.

첫 부팅 전에 호출자가 윈도우 D:에 새로운 빈 보존 디렉터리를 준비한다. 각 위치를 D:에 먼저 예약한다. 위치마다 ZIP과 개별 파일 명세·아카이브 해시를 즉시 D:에 저장한다. 닫은 ZIP을 다시 읽고, 윈도우에서도 실제 D: ZIP과 모든 멤버의 해시를 확인한다. 첫 위치의 두 검증 영수증이 있어야 두 번째 위치를 시작한다. 실패·부분 기록과 누락 파일, 재생을 실행할 수 없었던 사유도 보존한다. 전송·검증 실패 시 중단하며 이미 사용한 위치를 덮어쓰지 않는다. 남아 있는 스테이징 자료는 복구용으로 보존하고 누락 자료를 대신할 재실행은 하지 않는다.

## Fresh QA invocation / 독립 QA 실행

[#190 durable retention / 영구 보존](https://github.com/Romanof123/JNOX/issues/190) · [Parent / 상위 #6](https://github.com/Romanof123/JNOX/issues/6). Provision an empty D: directory outside this wrapper. Pass the retained #183 share/default input directory and the new destination; substitute actual paths. / 래퍼 외부에서 D:의 빈 디렉터리를 준비하고 실제 #183 입력 위치와 보존 위치를 전달한다.

```bash
/usr/bin/python3 tools/native_weighted_timer_pair.py /path/to/183/first/share-default /mnt/d/evidence/190-second --scratch /tmp
```

The wrapper records the two capture commands below and calls the official replay for each available printed digest. / 다음 두 캡처를 순서대로 기록하며 출력된 세션 해시가 있으면 각각 공식 재생을 실행한다.

```text
native_weighted_capture.py share default LOCAL/plain/session kernel.elf user.elf probe.c af952e34cea8fbe0aef5c7b4cb1f945a2884fff5
native_weighted_replay.py LOCAL/plain/session CAPTURE_PRINTED_SHA256
native_weighted_capture.py share default LOCAL/instrumented/session kernel.elf user.elf probe.c af952e34cea8fbe0aef5c7b4cb1f945a2884fff5 --timer-observation
native_weighted_replay.py LOCAL/instrumented/session CAPTURE_PRINTED_SHA256
```

A verified ZIP establishes retention only. A nonzero capture or replay, absent raw member, missing expected observer material or incomplete instrumented trace remains unqualified even when the archive verifies. / ZIP 무결성 확인은 자료 보존 판정이다. 캡처·재생 실패, 필수 원시 파일·계측 기록 누락 또는 불완전한 계측 추적은 보존 성공과 별도로 미충족이다.
