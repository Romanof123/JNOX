# Native display, X11 and kernel log results / 화면·X11·커널 로그 검증 결과

The AArch64 JNOX C application **JNOX X11 Mosaic** opens a real X11 window using the native IPv4 service. The reference X server runs outside the guest. With local presentation enabled, the guest reads the server's pixels, decodes the replies, and writes them to its own QEMU framebuffer. A click changes the lower tile from red to blue. A second click closes the window. The native C client and Rust network broker both exited 0.

AArch64 JNOX에서 실행한 C 앱이 네이티브 IPv4 서비스로 X11 창을 열었다. X 서버는 게스트 밖에서 실행했다. 로컬 출력 구성에서는 게스트가 서버에서 실제 픽셀을 받아 디코딩하고 자체 가상 프레임버퍼에 표시했다. 첫 클릭으로 아래쪽 타일이 빨강에서 파랑으로 바뀌고, 두 번째 클릭으로 창이 닫혔다. C 앱과 Rust 네트워크 서비스 모두 종료 코드 0을 반환했다.

| Before click / 클릭 전 | After click / 클릭 후 |
| --- | --- |
| ![QEMU guest, red tile](native-io-frames/guest-x11-before.png) | ![QEMU guest, blue tile](native-io-frames/guest-x11-after.png) |

## Observations / 실측

| Evidence | Target and result / 대상과 결과 |
| --- | --- |
| EVD-314 · [#406](https://github.com/Romanof123/JNOX/issues/406) | One privileged Rust device backend and one authorized ordinary Rust service. The service submitted 320-pixel rows, received `n` on the modeled PL011 and changed the numbered 320×240 image. QMP captured both full frames twice. A peer was denied access. The missing-device and headless controls returned their defined errors. [Before](native-io-frames/driver-before.png) · [After](native-io-frames/driver-after.png) |
| EVD-315 · [#407](https://github.com/Romanof123/JNOX/issues/407) | A native C client authenticated to Xvfb over JNOX TCP, mapped a 240×160 window and handled XTEST button input. An independent X client read the reference screen. It checked all 38,400 window pixels in both states, then confirmed removal. The earlier host packaging attempt stopped at the host linker before guest launch. Fixing the isolated host tool paths allowed the same client and broker ELFs to run. [Reference before](native-io-frames/reference-x11-before.png) · [Reference after](native-io-frames/reference-x11-after.png) |
| EVD-316 · [#408](https://github.com/Romanof123/JNOX/issues/408) | Three native shell boots covered a live serial prompt, a scripted pipeline and a child with no permission to read the journal. A live `dmesg` twice returned the identical boot and ready records. After child launches and exits the four-entry journal reported two overwritten records. The denied child exited 1. [Shell commands and supported events](../../examples/native-shell-pipeline/README.md) |
| EVD-317 · [#410](https://github.com/Romanof123/JNOX/issues/410) | The guest sent five X11 GetImage requests per frame. QMP read the complete 320×240 scanout twice per state. All 76,800 pixels matched the independently read X window centered at (40,40) with black margins. The external X server sent 307,884 bytes cumulatively by the second local capture. The modified client also passed its remote-only regression. [Build and run guide](../../examples/native-x11/README.md) |
| [#411](https://github.com/Romanof123/JNOX/issues/411) | Four additional first boots used the [resolved safety and display source tree](https://github.com/Romanof123/JNOX/commit/d7f1aa2403f2054581fb0ff388581b78e5ff0308). The ordinary FIFO safety run had sequence 4 queued at D, lowered the virtual output on expiry, and ended with the output low. The actual CHECK was 4,914 ticks (78.624 µs) after D. Seventeen queue observations and 167 protected records matched the independent decoder, with zero lost safety records. The general scheduler trace dropped 229 records. The separate interactive display, missing-device and headless runs also passed. |

화면 드라이버는 지정한 앱만 사용할 수 있으며 다른 프로세스의 호출은 거부됐다. 실제 PL011 입력 `n`으로 화면을 바꾼 후 모델 화면 전체를 대조했다. X11 시험은 서버 창의 38,400픽셀과 게스트 화면의 76,800픽셀을 상태별로 확인했다. 첫 호스트 패키징 시도는 게스트를 시작하기 전에 링크 오류로 중단됐고, 격리한 호스트 경로를 수정한 후 같은 앱·서비스 ELF로 검증했다. 셸은 최근 이벤트 네 건을 보존하며 덮어쓴 건수를 표시한다. 통합 소스에서 기존 안전 앱도 재검증했다. 실제 만료 판정이 78.624 µs 늦어 정각 조건은 여전히 열려 있다.

## Configuration and limits / 구성과 범위

- Native device testing used one Cortex-A72 on QEMU virt. EL1 configures QEMU ramfb through fw_cfg DMA and owns the scanout memory. The selected native EL0 service can submit one checked row and poll serial input. Physical display and input, driver restart/resource reclamation, an unmodified Linux driver domain and product safety qualification retain separate criteria. [Driver #5](https://github.com/Romanof123/JNOX/issues/5) · [Linux driver #3](https://github.com/Romanof123/JNOX/issues/3)
- The interactive shell supports these four native shell event kinds — native boot, shell ready, child launch and child exit. The existing UART stream and safety journal keep their own record formats. The snapshot is local to one boot.
- Xvfb served the native client through an authenticated, loopback-only test relay. The input used XTEST on the reference X server; the standalone driver test exercised serial input on JNOX. The screenshots establish functional output for two states. A complete desktop, frame-rate target, physical input device, native Wayland compositor and resident Xwayland remain open. [Resident window system #412](https://github.com/Romanof123/JNOX/issues/412)
- The four cross-profile boots used separate packages on the same integrated source. Timing interference with graphics active in a safety workload has not been measured. IPC-010 remains partial and its original release gates remain in [#401](https://github.com/Romanof123/JNOX/issues/401).

단일 가상 Cortex-A72에서 EL1이 실제 장치 레지스터와 스캔아웃 버퍼를 소유하고, EL0 앱은 허용된 행 단위 제출·입력 조회만 수행한다. 셸 로그는 한 번의 부팅에 속한다. X11 창 입력은 참조 서버의 XTEST로, 별도 드라이버 입력은 JNOX의 시리얼로 검증했다. 실보드와 기존 Linux 드라이버 이관, JNOX 상주 합성기 및 Xwayland, 그래픽 부하를 함께 실행한 안전 시간 보장은 각각 남아 있다.

## Raw data and replay / 원본 및 재현

| Packet | SHA-256 | Offline replay from a fresh extraction |
| --- | --- | --- |
| [Original driver, shell and reference X11](native-io-first.tar.xz) | `d19fa72becd70a6ef390568f1fd1ece25e5934c23e2f956fecb0d3dd7cff15d3` | `python3 -B io-first/replay.py` |
| [Local X11 presentation and remote-only control](native-x11-local-first.tar.xz) | `57e32cd9af976aa656a208199a43044f7608c40f65dc9ef6248c1a34cb2b7b59` | `sha256sum -c SHA256SUMS`, then `python3 -B replay.py` |
| [Merged safety and display captures](native-safety-display-integration.tar.xz) | `c38636169ae7bc14889d368535c30ecfa5764bb27b9c1117355a43edc0390934` | `bash replay.sh` |

The first driver and X11 packages were built from Git tree `f538bc1b0b7f6d8777df483d482615da9f2a9cb7`, the shell from `172025caa3df35e14d2571a157aa49d9ec86b0bd`, local X presentation from `d8c09d4189b9a72a5f8ed157f1e9fd1322eae446`, and cross-profile boots from `57587b6a29de92b4dbe43ace89698596f6c1035d`. X11 captures retain package digests and UART image identities. The short-lived, per-run credentials and the temporary kernel image embedding them are omitted. The guest client, network service and build commands are available for fresh authenticated runs. [First display/X11 source](https://github.com/Romanof123/JNOX/commit/669d88c1bd2e12434bab3fe09c9ebeaf726ebab5) · [Shell](https://github.com/Romanof123/JNOX/commit/d9e098f856a6b30f0f31dea5cc28c05b8f69980a) · [Local presentation](https://github.com/Romanof123/JNOX/commit/fe2fb3f477f9bbfa48f572ef34430f190e2f151b) · [Integration](https://github.com/Romanof123/JNOX/commit/d7f1aa2403f2054581fb0ff388581b78e5ff0308)

패킷을 각각 빈 디렉터리에 풀어 표의 명령을 실행한다. X11 자료에는 패키지 해시와 UART 이미지 식별자를 보존했다. 실행마다 생성된 인증값과 이를 포함하는 임시 커널 이미지는 배포 자료에서 제외했다. 새 인증값을 사용한 실행은 연결한 가이드로 재현할 수 있다.
