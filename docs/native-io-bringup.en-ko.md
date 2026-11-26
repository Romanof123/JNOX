# Native display, input and X11 client bring-up / 네이티브 화면·입력 및 X11 클라이언트

The first target is AArch64 QEMU virt, one Cortex-A72 vCPU. The optional native display service will own a 320×240, 32-bit XRGB software scanout and receive serial keyboard bytes. QEMU ramfb reads a kernel-owned framebuffer. Only the designated EL0 service may initialize scanout, submit one bounded row, or poll the PL011 receiver. A second process exercises the ownership boundary. Keep headless images selectable and test the no-device case. Compare two external QMP screenshots to independent pixel expectations before and after a known input. Physical panel, USB input, IRQ/DMA lifecycle and restart qualification remain on the driver roadmap. [Native driver #5](https://github.com/Romanof123/JNOX/issues/5) · [Model driver #406](https://github.com/Romanof123/JNOX/issues/406) · [QEMU firmware interface](https://www.qemu.org/docs/master/specs/fw_cfg.html)

첫 보드는 단일 Cortex-A72 가상 시스템이다. 지정된 Rust 사용자 서비스가 커널의 제한된 장치 인터페이스로 320×240 화면을 갱신하고 PL011 입력을 읽는다. 모델 화면 자체를 캡처하여 입력 전후의 픽셀을 검증한다. 장치 누락·소유권·헤드리스 구성도 별도로 확인한다.

A named native X11 client will use the existing JNOX IPv4 socket service to create, map and draw a window on an authenticated reference X server. A separate host client observes its actual window and sends a button event. Verify the changed pixels and normal termination from both sides. This exercises a remote X display. The local native compositor, focus across isolated native clients and a resident Xwayland server remain later stages of the [window-system plan](window-system-plan.en.md). [X11 integration #407](https://github.com/Romanof123/JNOX/issues/407)

네이티브 X11 클라이언트는 JNOX 소켓을 통해 독립 X 서버에 직접 연결한다. 창 생성·표시·입력에 따른 재그리기를 호스트에서 확인한다. JNOX 자체 출력의 로컬 합성기와 Xwayland는 창 시스템 계획에서 계속 추적한다.

The native shell will gain read-only access to bounded kernel diagnostics under its selected profile. Exercise the real command, event ordering and output through the shell. The separate safety IPC criteria, including native expiry at the actual decision tick, remain tracked on the safety branch. [Shell diagnostics #408](https://github.com/Romanof123/JNOX/issues/408) · [Safety IPC #401](https://github.com/Romanof123/JNOX/issues/401)

네이티브 셸에서 읽기 전용으로 커널 진단 기록을 확인한다. IPC-010의 실제 만료 판정과 제품 안전 승인 조건은 기존 이슈에서 유지한다.
