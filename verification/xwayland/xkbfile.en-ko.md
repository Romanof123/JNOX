# libxkbfile for native Xwayland / 네이티브 Xwayland용 libxkbfile

The pinned Xwayland 24.1.6 source requests `dependency('xkbfile')` in its top-level Meson build. libxkbfile's own Meson project compiles thirteen original translation units. Its public package requires kbproto; its private static-link dependency is X11. Use the existing graphics runtime's Picolibc, native startup, AArch64 compiler options and linker script, together with the prepared X.Org protocol and Xtrans headers. [X.Org releases](https://www.x.org/releases/individual/lib/) · [XCB releases](https://xcb.freedesktop.org/dist/) · [Input hashes](xkb-sources.sha256)

| Upstream input | Role |
| --- | --- |
| libxkbfile 1.1.3 | Keymap file, rules, atom, text and keyboard geometry implementation |
| libX11 1.8.10 | Public Xlib and XKBlib headers and the target implementation of Xlib/XKB calls |
| libXau 1.0.11 and libxcb 1.17.0 | Authorization and X protocol transport required by upstream Xlib |
| xcb-proto 1.17.0 | Host-side Python generator and XML protocol descriptions |
| libpthread-stubs 0.5 | Upstream configure checks and pkg-config metadata only. Any pthread operation needed by target code must come from the target runtime. |

## Build in a serial gate

Start from the output of the [graphics runtime build](../../sdk/native-aarch64/graphics/README.md) and the target prefix produced by [protocol preparation](dependencies.md). Use a Linux shell, Clang, LLVM binutils, LLD, GNU binutils, Make, Ninja, Meson, pkg-config, patch, Python 3 and the host build tools requested by upstream configure. The supplied runtime directory must retain the absolute paths baked into its generated `jnox.ini`. Set `CACHE`, `RUNTIME`, `PROTOCOL_TARGET`, `XWAYLAND_ARCHIVE` and fresh absolute output paths. The source download is independent of compilation.

```sh
sh verification/xwayland/xkb-fetch.sh "$CACHE"
python3 verification/xwayland/xkb-build.py --cache "$CACHE" --runtime "$RUNTIME" \
  --protocols "$PROTOCOL_TARGET" --out "$NEW_ARCHIVE_WORK"
python3 verification/xwayland/xkb-build.py --cache "$CACHE" --runtime "$RUNTIME" \
  --protocols "$PROTOCOL_TARGET" --server "$XWAYLAND_ARCHIVE" --out "$NEW_SERVER_WORK"
```

The first invocation checks the source and runtime hashes, compiles every original libxkbfile source from the upstream list, verifies one AArch64 object per translation unit and links `probe-archive.elf`. The probe allocates and frees an upstream XKB rules structure and checks that modifier index 2 formats as “Control”. The native link map identifies the library objects selected for those operations. This stage leaves the static archive private to its output directory and does not install x11.pc or xkbfile.pc. Cross-linking the probe establishes symbol resolution. Its functional result requires executing the target ELF and observing exit zero.

The second invocation also builds the real static dependency chain, checks the target archive members, and uses upstream Meson to install libxkbfile and its original package metadata. It requires the x11, xau, xcb and xkbfile metadata providers to resolve under its new target prefix. A second native probe calls the real Xlib `XkbLibraryVersion` and the libxkbfile operations. Finally the command checks the original Xwayland archive digest, applies the repository's JNOX transport patch and runs the real software-profile Meson configure. Keep `probe-x11.map`, `xwayland-build/meson-logs/meson-log.txt`, the complete command transcript and first error. If target transport, threading or filesystem support is absent, preserve that failure. The Meson lookup, runnable library probe, final server link, xkbcomp loading a real keymap and guest X11 session are separate observations.

## 순차 빌드와 증거

먼저 Picolibc 그래픽 런타임과 X.Org 프로토콜·Xtrans 헤더를 준비한다. 위 명령의 `RUNTIME`은 원래 절대 경로에 있는 런타임 출력 디렉터리이며, `PROTOCOL_TARGET`은 프로토콜 준비 스크립트의 target 디렉터리다. `XWAYLAND_ARCHIVE`는 원본 Xwayland 24.1.6 tar.xz 파일이다. 아카이브 단계와 전체 서버 단계에는 서로 다른 새 작업 디렉터리를 사용한다.

첫 단계는 libX11 원본 헤더로 libxkbfile의 원본 C 파일 13개를 모두 컴파일한다. AArch64 오브젝트 수와 기종을 검사하고 실제 규칙 할당·해제 및 수정키 이름 변환 함수를 호출하는 ELF를 링크한다. 타깃 실행 시 종료 코드 0을 확인해야 동작 검증이 끝난다. 두 번째 단계는 Xau, XCB, X11을 원본에서 정적으로 빌드한다. libpthread-stubs 0.5는 구성 정보만 제공하며 실제 pthread 함수가 필요하면 타깃 런타임이 구현해야 한다. libxkbfile을 자체 Meson 프로젝트로 설치한 뒤 실제 Xlib의 버전 조회 함수까지 링크한다. 이후 고정된 원본 서버 소스에 저장소 패치를 적용해 Xwayland의 Meson 구성을 실행한다.

대상 시스템에 필요한 소켓·스레드·파일 기능이 없으면 최초 오류와 전체 명령 로그를 보존한다. 대상 라이브러리 검색 결과, 타깃 ELF 실행, 서버 링크, 실제 키맵 로딩, 게스트 X11 세션은 각각 별도로 확인한다.
