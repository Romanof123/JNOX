# Xwayland source integration / Xwayland 소스 통합

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. The target is the upstream Xwayland 24.1.6 server communicating with the native compositor and accepting the Mosaic X11 client. The three dependency candidates contribute source preparation and target probes. These have separate acceptance criteria from a linked server, a running guest, and a visible X11 frame. [Server acceptance #448](https://github.com/Romanof123/JNOX/issues/448) · [Integration #452](https://github.com/Romanof123/JNOX/issues/452)

## Build order

Use one Linux gate at a time and preserve each first failure and its command log. Start with the original, verified 6764dfe graphics-runtime output and a fresh copy of the relocatable xorgproto/xtrans prefix. The runtime's `jnox.ini` points to its original absolute compiler and linker inputs. Keep that tree at its original location. Distinguish host generators from target archives.

```sh
set -eu
tool=verification/xwayland
export JNOX_GRAPHICS_RUNTIME="$RUNTIME"
sh "$tool/xkb-fetch.sh" "$XKB_CACHE"
python3 "$tool/xkb-build.py" --cache "$XKB_CACHE" --runtime "$RUNTIME" \
  --protocols "$PROTO" --out "$NEW_XKB_PROBE"
```

1. **XKB archive.** Check the target ELF that links the original rules and modifier functions. Save its link map. [XKB instructions](xkbfile.en-ko.md)
2. **Bitmap font.** Seed a new font prefix from the relocatable protocol target. Compile zlib, libfontenc and libXfont2 in that prefix. The probe reads the real embedded PCF for `6x13`. Its 13 pixel rows and actual native exit are separate guest gates. [Font instructions](font-builtins.md)
3. **Wayland client.** Build the host scanner, install protocol XML, then build target libffi and libwayland-client. The target probe requires a connected socket in fd 3 for a guest roundtrip. Keep the host scanner and its metadata out of target library lookup. [Wayland instructions](client/README.md)
4. **Server configure.** Build the original X11/XCB stack and complete libxkbfile in a fresh work directory. The final invocation supplies the font and client target prefixes and the independent native scanner configuration to the actual server Meson build. Preserve its log, including any next missing package. The full server link and guest invocation follow after its remaining dependencies are satisfied.

```sh
mkdir "$NEW_FONT_PREFIX"
cp -a "$PROTO"/. "$NEW_FONT_PREFIX"/
TARGET_CC="$(realpath "$tool/graphics-cc.py")" sh "$tool/font-static.sh" \
  "$FONT_CACHE" "$NEW_FONT_WORK" "$NEW_FONT_PREFIX"
sh "$tool/client/fetch.sh" "$WAYLAND_CACHE"
for phase in prepare protocols ffi client; do
  python3 "$tool/client/build.py" "$phase" "$WAYLAND_CACHE" "$RUNTIME" "$NEW_CLIENT_WORK"
done
python3 "$tool/xkb-build.py" --cache "$XKB_CACHE" --runtime "$RUNTIME" \
  --protocols "$NEW_FONT_PREFIX" --wayland-work "$NEW_CLIENT_WORK" \
  --server "$XWAYLAND_ARCHIVE" --out "$NEW_SERVER_WORK"
```

## 한국어

원래 경로에 있는 6764dfe 그래픽 런타임과 xorgproto/xtrans 타깃 접두 디렉터리에서 시작한다. 먼저 XKB 규칙 함수의 실제 정적 링크를 확인한다. 다음으로 프로토콜 파일을 복제한 전용 접두 디렉터리에 글꼴 라이브러리를 설치한다. Wayland는 호스트 scanner와 프로토콜을 먼저 준비하고, 타깃 libffi와 클라이언트 라이브러리를 순서대로 빌드한다. 마지막 명령은 이렇게 생성한 타깃 라이브러리와 별도의 호스트 scanner 설정을 실제 서버의 Meson 구성에 제공한다. 단계마다 로그와 첫 실패를 보존한다. 각 프로브의 게스트 실행, 실제 X11 접속, Mosaic 화면 표시는 후속 실행 증거로 확인한다.
