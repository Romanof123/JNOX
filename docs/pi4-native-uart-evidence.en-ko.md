# Pi 4 native prime-pair UART evidence / Pi 4 네이티브 소수 앱 UART 근거

CHG-301 connects POL-002 and REQ-023 to the native UART observation under VER-023. Two additional profiles use the ordinary serial collector's physical-port and file-replay entry points. The three existing linked-image profiles retain their original matching rules. The host acceptance inputs are the four original EVD-129 UART streams, plain and traced for each policy. [#364](https://github.com/Romanof123/JNOX/issues/364) · [EVD-129 first positions](evidence/pi4-native-fixed-applications-first.en-ko.md) · [Serial collector](boards/pi4-serial-capture.en.md)

CHG-301은 POL-002·REQ-023의 VER-023 관측에 네이티브 UART를 추가한다. 기존 연결형 이미지 세 프로필과 판정 규칙을 유지하고, 같은 실포트·파일 재생 경로에서 FIFO·RR 네이티브 프로필 둘을 제공한다. 호스트 수용 입력은 EVD-129의 정책별 일반·추적 UART 원본 네 개다.

## Capture and acceptance / 수집과 판정

Bind the Pi 4 package manifest, selected policy, exact kernel ELF and raw boot image, and both ordered application ELF digests. The UART must independently report those app digests in order. Both tasks have priority 2. PID 0 runs the C prime counter (78,498, exit 12); PID 1 runs the Rust counter (70,435, exit 77). RR uses the packaged quantum of 1,250,000 counter ticks. Archive raw bytes, acquisition mode, capture identity, package hashes, both PID exits, complete fixed-event records and the explicit dropped count. Retain partial bytes and the reason for any incomplete or failed capture. A replay must identify its input as a file even when those bytes originally came from a board.

For FIFO, require the reported initial choice of PID 0, retention through its exit, handoff to PID 1 and both exits. For RR, require timer-driven 0→1→0 handoffs before either task exits. Read each transition using its cause, outgoing and selected owner, counter/compare and frame fields. A same-owner interrupt is not a rotation. Accept different monotonic counter values and different numbers of observed rotations; the archived count of 16 is a measurement. An explicitly dropped suffix can leave an earlier witnessed RR alternation intact. State exactly what the retained prefix establishes. If a gap could conceal a FIFO rotation, or the record cannot establish required ordering, identity, termination or release, leave the corresponding conclusion inconclusive.

패키지 매니페스트의 Pi 4 정책·커널 ELF·원시 부트 이미지·순서가 고정된 앱 ELF 둘의 해시를 대조하고, UART가 보고한 앱 해시 순서도 확인한다. 같은 우선순위 2에서 PID 0의 C 앱은 소수 78,498개·종료값 12, PID 1의 Rust 앱은 70,435개·종료값 77이다. RR 양자는 1,250,000틱이다. 원시 바이트, 획득 방식과 식별자, 패키지 해시, 두 종료 기록, 온전한 이벤트 행과 누락 수를 남긴다. 실패·중단 시 받은 바이트와 사유를 보존하고 재생 출처를 명시한다.

FIFO는 PID 0 최초 선택부터 해당 종료까지 유지되고 PID 1로 넘어가 종료하는지 확인한다. RR은 두 앱이 모두 살아 있을 때 타이머 원인의 0→1→0 전환을 요구한다. 같은 PID 복귀는 회전에 포함하지 않는다. 행의 원인·소유자·카운터·비교값·프레임으로 확인하며 모델과 동일한 절대 틱이나 16회 회전을 요구하지 않는다. 뒤쪽 누락 이전에 이미 확보한 RR 교대 근거는 보존한다. FIFO 이탈을 숨길 수 있는 공백이나 식별·종료·순서·릴리스를 확인할 수 없는 경우 그 항목은 판정 보류다.

## Negative controls and proof boundary / 음성 대조와 근거 범위

Exercise swapped policy, reversed or substituted app, changed kernel image/ELF, inconsistent manifest, missing or duplicate exit, malformed event, impossible owner transition, incomplete final line and absent footer. Preserve the distinction between an image association failure, contradictory complete observations and insufficient bytes. Fragmenting an unchanged stream at a host read boundary must preserve its interpretation. Simulated port delivery and all four archived model replays remain host evidence.

UART supports the decisions and statuses the kernel reported. The expected prime counts were independently checked in EVD-129 model memory. Physical private RAM values, actual interrupted and restored execution, physical counter timing, firmware boot and power-cycle provenance require their own board evidence. Host USB arrival timestamps cannot substitute for those observations. Board and safety/product decisions remain with #34 and #21; #365 authorizes IPC engineering independently. [#34](https://github.com/Romanof123/JNOX/issues/34) · [#21](https://github.com/Romanof123/JNOX/issues/21) · [#365](https://github.com/Romanof123/JNOX/issues/365)

정책 교환, 앱 교체·순서 반전, 커널 ELF·이미지 변경, 매니페스트 모순, 종료 누락·중복, 잘못된 행·소유자 전환, 마지막 줄 잘림과 끝 기록 누락을 대조한다. 파일 결합 실패, 실제 관측의 모순, 바이트 부족을 구분한다. 읽기 경계만 달라진 동일 바이트는 같은 판정이어야 한다. 포트 모의 입력과 모델 재생은 호스트 근거다. UART는 커널이 보고한 결정과 상태를 뒷받침한다. 실보드 메모리 산출물, 실제 중단·복귀, 타이머 시각과 전원·펌웨어 부팅은 별도 근거가 필요하다. USB 도착 시각으로 이를 대신할 수 없다. #34·#21의 보드 및 안전·제품 승인은 별도이며 #365의 IPC 개발 권한은 유지된다.

## Host tool release / 호스트 수집기 검증

**IMP-301 · EVD-301.** The independent verifier replayed all four EVD-129 model streams as files and through a simulated serial adapter. A disconnect and a keyboard interrupt after 62 bytes retained the exact input prefix and its digest, the error and the run identity. The corrected collector passed 17 host tests. Physical power-on, timer interrupts and CPU return context remain with #34. [First failure](evidence/pi4-native-serial-first.en-ko.md) · [Corrected evidence](evidence/pi4-native-serial-corrected.en-ko.md)

원본 모델 로그 4개를 파일과 모의 직렬 입력으로 각각 확인했다. 62바이트를 받은 뒤 연결이 끊기거나 사용자가 중단해도 수집기가 해당 바이트의 해시·원인·실행 식별자를 남겼다. 호스트 시험 17개가 통과했고 실보드 전원 기동·타이머·선점 검증은 #34에 남아 있다. [독립 재검증](evidence/pi4-native-serial-corrected.en-ko.md)
