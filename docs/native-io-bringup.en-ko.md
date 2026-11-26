# Native display, input and X11 client bring-up / 네이티브 화면·입력 및 X11 클라이언트

The first target is AArch64 QEMU virt, one Cortex-A72 vCPU. The optional native display service will own a 320×240, 32-bit XRGB software scanout and receive serial keyboard bytes. QEMU ramfb reads a kernel-owned framebuffer. Only the designated EL0 service may initialize scanout, submit one bounded row, or poll the PL011 receiver. A second process exercises the ownership boundary. Keep headless images selectable and test the no-device case. Compare two external QMP screenshots to independent pixel expectations before and after a known input. Physical panel, USB input, IRQ/DMA lifecycle and restart qualification remain on the driver roadmap. [Native driver #5](https://github.com/Romanof123/JNOX/issues/5) · [Model driver #406](https://github.com/Romanof123/JNOX/issues/406) · [QEMU firmware interface](https://www.qemu.org/docs/master/specs/fw_cfg.html)

첫 보드는 단일 Cortex-A72 가상 시스템이다. 지정된 Rust 사용자 서비스가 커널의 제한된 장치 인터페이스로 320×240 화면을 갱신하고 PL011 입력을 읽는다. 모델 화면 자체를 캡처하여 입력 전후의 픽셀을 검증한다. 장치 누락·소유권·헤드리스 구성도 별도로 확인한다.

A named native X11 client will use the existing JNOX IPv4 socket service to create, map and draw a window on an authenticated reference X server. A separate host client observes its actual window and sends a button event. Verify the changed pixels and normal termination from both sides. This exercises a remote X display. The local native compositor, focus across isolated native clients and a resident Xwayland server remain later stages of the [window-system plan](window-system-plan.en.md). [X11 integration #407](https://github.com/Romanof123/JNOX/issues/407)

네이티브 X11 클라이언트는 JNOX 소켓을 통해 독립 X 서버에 직접 연결한다. 창 생성·표시·입력에 따른 재그리기를 호스트에서 확인한다. JNOX 자체 출력의 로컬 합성기와 Xwayland는 창 시스템 계획에서 계속 추적한다.

The native shell will gain read-only access to bounded kernel diagnostics under its selected profile. Exercise the real command, event ordering and output through the shell. The separate safety IPC criteria, including native expiry at the actual decision tick, remain tracked on the safety branch. [Shell diagnostics #408](https://github.com/Romanof123/JNOX/issues/408) · [Safety IPC #401](https://github.com/Romanof123/JNOX/issues/401)

네이티브 셸에서 읽기 전용으로 커널 진단 기록을 확인한다. IPC-010의 실제 만료 판정과 제품 안전 승인 조건은 기존 이슈에서 유지한다.

## Implementation and evidence / 구현 및 관측 근거

The issue rows below use the same suffix for the change (CHG), implementation (IMP) and evidence (EVD). The capture report identifies the source and outcome for each run. All guest observations apply to one emulated AArch64 CPU. [Capture report / 실행 기록](evidence/native-display-x11-shell.en-ko.md) · [Trace links / 추적 관계](../trace/links.csv)

아래 표의 CHG는 변경, IMP는 구현, EVD는 해당 구현의 관측 기록이다. 실행별 소스와 판정은 위 보고서에서 확인할 수 있다. 게스트 관측 범위는 단일 가상 AArch64 CPU다.

| Issue / 이슈 | Change · implementation · evidence | Observation / 관측 범위 |
|---|---|---|
| [#406](https://github.com/Romanof123/JNOX/issues/406) | CHG-314 · [IMP-314](../examples/native-display/src/main.rs) · EVD-314 | QMP captured both frames around PL011 input. The peer could not use the display ABI. Separate boots covered missing ramfb and the same demo ELF in a headless profile. / PL011 입력 전후 화면을 QMP로 캡처했다. 다른 프로세스의 장치 접근 거부, ramfb 누락, 같은 ELF의 헤드리스 실행도 확인했다. |
| [#407](https://github.com/Romanof123/JNOX/issues/407) | CHG-315 · [IMP-315](../examples/native-x11/client.c) · EVD-315 | The native C client opened JNOX X11 Mosaic on a private reference Xvfb server. A host observer checked the window and its change after a button press. / 게스트 C 클라이언트가 전용 Xvfb에 연 창과 버튼 입력으로 바뀐 픽셀을 호스트에서 확인했다. |
| [#408](https://github.com/Romanof123/JNOX/issues/408) | CHG-316 · [IMP-316](../kernel/src/native_shell_klog.rs) · EVD-316 | UART from the interactive, scripted and denied-caller boots records the shell’s four-entry kernel journal, overwrite count and permission boundary. / 대화형 셸, 스크립트 및 권한 없는 호출자 부팅의 UART에서 네 건짜리 커널 기록, 덮어쓴 건수와 접근 제한을 확인했다. |
| [#410](https://github.com/Romanof123/JNOX/issues/410) | CHG-317 · [IMP-317](../examples/native-x11/README.md) · EVD-317 | The guest read the X window in five `GetImage` strips per frame. Both 320 × 240 QMP captures matched the expected placement of the reference window before and after XTEST input. A subsequent remote-only run also passed. / 게스트가 프레임마다 X 창의 픽셀을 다섯 번에 나누어 읽었다. XTEST 입력 전후 모두 320 × 240 QMP 캡처가 X 창의 픽셀과 지정된 배치에 일치했고, 이어서 원격 출력 전용 실행도 통과했다. |

The resident compositor, two local clients and guest Xwayland share a separate acceptance plan. 상주 컴포지터, 로컬 클라이언트 두 개와 게스트 Xwayland의 검증 기준은 별도 이슈에서 추적한다. [Resident window system / 상주 윈도 시스템 #412](https://github.com/Romanof123/JNOX/issues/412)
