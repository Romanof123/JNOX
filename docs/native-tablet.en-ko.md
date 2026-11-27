# CHG-341 · Native tablet input / 네이티브 태블릿 입력

REQ-015/016/024 · DES-009/013 · VER-015/016/024. [Device and pointer acceptance #451](https://github.com/Romanof123/JNOX/issues/451) · [Measured QEMU device, windows and open protocol finding](evidence/native-tablet/README.md)

SVC 197 belongs to the display-owner tablet interface. Shared-file operations reserve 189 through 196. The caller sets x8 to 197 and executes the ordinary native SVC. With x0 zero, the owner acquires or probes the device. With x0 one, it reads one packed event. Bits 63–48 carry the input type, 47–32 the code, and 31–0 the value. A zero result on read is SYN_REPORT. Empty reads return -11, unauthorized callers receive -1, an absent tablet returns -19, and a broken transport returns -5. Packages built without the tablet return -38.

SVC 197은 디스플레이 소유자의 태블릿 호출이다. 공유 파일 호출에는 189~196을 배정한다. x8에 197을 넣고 x0에 0을 넣으면 장치 확인 및 획득, 1을 넣으면 이벤트 하나를 읽는다. 상위 16비트는 종류, 다음 16비트는 코드, 하위 32비트는 값이다. 읽기 결과 0은 SYN_REPORT이고 -11은 빈 큐다. 권한이 없으면 -1, 장치가 없으면 -19, 전송 오류는 -5, 태블릿 기능이 없는 패키지는 -38을 반환한다.

The opt-in package switch is `--pointer-tablet`. QEMU needs `-global virtio-mmio.force-legacy=false -device virtio-tablet-device`. The driver reads the actual ABS_X/Y limits and BTN_LEFT bitmap, and maps the axes to 0–319 and 0–239. EL1 owns both DMA queues and masks the transport SPI. It polls completions, acknowledges ISR bits and releases queue memory only after reset confirms both queues are inactive. Malformed completions, SYN_DROPPED and configuration changes quarantine the device. Every read processes at most 16 new descriptors.

`--pointer-tablet`로 빌드하고 QEMU에 위 장치를 추가한다. 드라이버는 장치에서 ABS_X/Y 범위와 BTN_LEFT 비트를 읽어 320×240 출력 좌표로 변환한다. EL1이 DMA 큐 두 개를 소유하고 해당 SPI를 마스킹한 상태에서 완료 항목과 ISR을 확인한다. 리셋 후 두 큐가 비활성임을 확인해야 메모리를 회수한다. 잘못된 완료 항목, SYN_DROPPED 또는 설정 변경이 발생하면 장치를 격리한다. 한 번에 새 항목을 최대 16개 처리한다.

When two windows are mapped, run `python3 tools/native_tablet_qmp.py QMP_SOCKET KERNEL_ELF NEW_OUTPUT_DIR` on the QEMU host. It injects two motion/down/up sequences and saves each QMP request, device receipt and DMA queue. It requires a fresh acquisition with no other input producer, an active absolute QEMU pointer and a shared host filesystem for QMP pmemsave. Verify the Wayland wire and application effect separately.

두 창이 매핑되면 QEMU 호스트에서 위 명령을 실행한다. 다른 입력원이 없는 새 장치 세션에서 두 위치에 이동·누름·해제를 전달하고 QMP 요청, 장치 수신 기록, DMA 큐를 저장한다. Wayland 메시지와 앱의 반응은 별도로 대조한다.
