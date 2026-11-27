# Wayland reference client / Wayland 참조 클라이언트

On amd64 Linux this fixture uses Ubuntu's unmodified libwayland-client to send standard requests over a real Unix stream to the JNOX compositor executable. The executable uses the Linux transport and framebuffer adapters with the production dispatcher and renderer. The QEMU boot measures the JNOX broker, descriptor transfer, guest mappings and display separately. [Compositor acceptance #446](https://github.com/Romanof123/JNOX/issues/446) · [Compositor design](../../docs/native-wayland-compositor.en-ko.md)

## Reproduce on Ubuntu 22.04 amd64

Use Python 3.10+, a C compiler, dpkg-deb, coreutils and the system runtime dependencies of the pinned libraries and scanner. Each output directory below must be new. `server` points to the Linux host executable built from the JNOX compositor revision being measured. Run from this directory.

```sh
server=/absolute/path/to/the/jnox-compositor
work=$(mktemp -d)
python3 inputs.py prepare "$work/intake"
sh build-reference.sh "$work/intake/sysroot" "$work/build"
python3 run-reference.py "$server" "$work/build/reference-client" "$work/intake/sysroot" "$work/report"
cat "$work/report/result.json"
```

For offline reproduction, append `--debs /path/to/archives` to the preparation command. Ubuntu's APT catalog lists each archive filename and SHA-256. For example, `apt-cache show libwayland-client0=1.20.0-1ubuntu0.1` prints both. `upstream.json` records the original package paths, six archive hashes, both protocol XML hashes, scanner, library, selected header and copyright hashes. The preparation step checks each archive before extracting it under the destination and checks the selected files. The compiler uses fresh, unedited scanner output for the core client header and stable xdg-shell C bindings. Core interface symbols come from the distribution library. License notices are retained in the XML and in the three copyright files listed in the manifest. [Ubuntu Wayland package](https://launchpad.net/ubuntu/+source/wayland/1.20.0-1ubuntu0.1) · [Ubuntu wayland-protocols package](https://launchpad.net/ubuntu/+source/wayland-protocols/1.25-1) · [Ubuntu libxkbcommon package](https://launchpad.net/ubuntu/+source/libxkbcommon/1.4.0-1)

The runner starts the supplied server on a fresh socket with a new 320×240 raw framebuffer. The client binds compositor v4, shm v1, xdg_wm_base v1 and seat v4, receives both mandatory pixel formats and parses the keyboard's transferred XKB map with the packaged libxkbcommon. After receiving and acknowledging the initial xdg configure, it transfers a read-only FD for a 4 KiB shared backing and destroys its pool proxy while the buffer remains live. The 20×16 XRGB8888 image starts at byte 64 and uses stride 96. After the frame callback, the runner reads the server's external framebuffer and requires a unique match for all 320 RGB pixels before resuming the client. The client requires buffer release and a successful final display sync. [Wayland client API](https://wayland.freedesktop.org/docs/html/apb.html)

The report contains the preserved mapped frame, stock client's wire log, client and server logs, loaded library mappings, hashes and matched origin. Its backend field is `linux-host`. To connect a separate orchestrator, run `reference-client --socket /absolute/path` or `reference-client --fd N` with a connected, inherited Unix-stream FD. With `--hold-frame` the client prints `FRAME` on stdout and waits for a newline on stdin so the external observer can capture the mapped pixels.

## 한국어

Ubuntu 22.04 amd64에서 실제 libwayland-client와 wayland-scanner를 사용한다. 위 명령은 여섯 패키지의 SHA-256을 확인한 뒤 작업 디렉터리에만 압축을 푼다. 두 프로토콜 XML에서 새로 생성한 바인딩과 배포판 라이브러리로 참조 클라이언트를 빌드한다. `upstream.json`에는 원본 아카이브, XML, 실행 파일, 라이브러리 및 저작권 고지의 해시가 있다.

검증기는 지정한 JNOX 합성기를 새 Unix 소켓으로 실행한다. 클라이언트는 실제 FD로 받은 키맵을 libxkbcommon으로 읽고, 초기 xdg configure에 응답한 뒤 4 KiB 버퍼의 읽기 전용 FD를 보낸다. 프레임 콜백이 오면 외부 검증기가 호스트 프레임버퍼에서 패딩과 오프셋을 고려한 320개 픽셀 전체를 대조한다. 버퍼 반환과 최종 동기화까지 성공해야 결과 파일을 쓴다. 호스트 결과의 범위는 공통 디스패처와 렌더러의 상호운용성이다. 게스트 브로커, 화면·입력, 별도 앱 주소 공간과 재시작은 독립적인 게스트 실행으로 확인한다.
