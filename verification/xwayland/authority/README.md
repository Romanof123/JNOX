# Native Xauthority for Xwayland 24.1.6

Select `JNOX_NATIVE_XAUTHORITY=1` for server-lab.sh, or pass
`--native-xauthority` to the Picolibc overlay installer and the executable
server builder. This requires the SVC 199 boot-resource API and the #464
identity implementation. Use `:0 -auth /run/Xauthority` at startup.
The opt-in target links a separate libjnox-xauthority.a.

The original server polls stat(2) for mtime, opens the path with Fopen, loops
over XauReadAuth, and can enable local access for an empty or missing file.
ClientAuthorized also contains independent trusted-transport and host access
paths. The selected native server instead opens its immutable, role-specific
resource on each server generation after client resources are initialized.
SVC 199 supplies the size and complete bytes. Picolibc fmemopen supplies the
read-only FILE consumed by original libXau. No global file descriptor, invented
mtime, or writable file is involved. The caller owns the bounded backing
buffer until fclose. The native client independently reads its own resource.

Startup requires exactly one fully consumed FamilyLocal record, the real
uname nodename, the numeric display string selected by the server, the MIT
protocol name, and 16 bytes of private data. Trailing data, including a second
record, fails startup. Only after successful stream close does the server
call upstream MitAddCookie. Every client must pass upstream MitCheckCookie for
the ID installed by this loader. Failure returns a generic reason before local
host access can approve a connection. The ordinary build uses upstream policy.

## Reproduce #465 on AArch64

Use Xwayland 24.1.6 with this repository's `native-xauthority.patch`
applied, the graphics cross file, the staged target libXau and X11 headers,
and a Picolibc prefix populated by the native Xauthority installer. Run on
Linux with the repository's AArch64 C/Rust tools and QEMU. Choose fresh
output directories and pass the same lock used for the other guest runs.

```sh
auth=verification/xwayland/authority
python3 "$auth/package.py" --xwayland "$XWAYLAND_SRC" --cross "$RUNTIME/jnox.ini" \
    --xcb "$XCB_PREFIX" --protocol "$PROTOCOL_PREFIX" --os-prefix "$OS_PREFIX" \
    --out "$WORK/package-build" --lock "$WORK/run.lock"
python3 "$auth/boot.py" "$PWD" "$WORK/package-build/package/kernel.elf" \
    "$WORK/results" "$WORK/run.lock"
```

The listener calls the pinned native loader, original cookie insertion and
comparison, and original libXau. The test supplies display 0, a fixed test
resource ID and fatal-exit handling. Slots 0 and 2 test denied access, slot 1
runs the listener, slot 3 runs the socket broker, and slot 4 runs Mosaic.
The launcher creates fresh private credentials and saves only exit codes.
Two successive three-boot runs produced the following outcomes.

| Boot inputs | Slots 0, 1, 2, 3, 4 | Application observation |
| --- | --- | --- |
| Matching records | 72, 70, 72, 0, 104 | The original checker granted access. The fixture closed before a successful X11 setup reply and Mosaic reported a short reply. |
| Different client cookie | 72, 71, 72, 0, 114 | Mosaic received the failed setup reply and matched the original invalid-cookie reason. |
| Both records absent | 72, 91, 72, 0, 116 | Server initialization stopped. Mosaic reported its missing resource. |

## Remaining full-server verification

1. Generate private role files via the Mosaic prepare command in a private
   directory. Package each in its designated role. Keep binaries, traces and
   package files containing record bytes private.
2. Exercise stream-probe.c in an authorized slot. Run provider negatives for
   denied role, absent resource, undersized buffer and invalid destination.
3. On the real five-role guest, require a successful X11 setup and actual
   Mosaic rendering for matching private files. With two independently
   generated records, require a failed X11 setup on the same AF_UNIX listener.
   Repeat with missing client protocol and wrong protocol.
4. At server startup try missing, empty, truncated, overlong, duplicate,
   trailing garbage, wrong family, wrong node, wrong display and wrong data
   length inputs. Each must terminate before serving requests. Keep bounded
   ordinary outcomes and diagnostics. Never publish authorization bytes or
   dumps of processes and package files.

## 한국어

선택한 네이티브 서버는 부팅 리소스에서 자신의 권한 파일 길이와 실제
바이트를 받아 읽기 전용 Picolibc 스트림으로 연다. 매 서버 세대마다 원본
XauReadAuth로 레코드 하나를 읽으며 실제 노드 이름, 디스플레이, 프로토콜과
데이터 길이를 확인한다. 불완전하거나 추가 데이터가 있으면 시작을 중단한다.
클라이언트는 원본 MIT 쿠키 검사를 통과해야 한다. 로컬 연결도 동일하게
검사한다. 검증에는 정상 연결과 실제 렌더링, 서로 다른 비밀값의 접속 거부,
손상된 파일에 대한 시작 실패가 필요하다. 공개 기록에는 일반 실행 결과만
남기고 권한 데이터와 프로세스 덤프는 포함하지 않는다.

위 절차는 AArch64 QEMU에서 다섯 프로세스를 구동한다. 일치·불일치·양쪽
누락 조건을 각각 두 번 재현했다. 정상 쿠키에서는 서버 측 검사가 승인한 뒤
시험 리스너가 연결을 닫았고, 잘못된 쿠키에서는 Mosaic가 원본의 인증 거부
사유를 확인했다. 양쪽 파일이 없으면 서버 초기화와 클라이언트의 리소스
읽기가 각각 실패했다. 전체 Xwayland의 정상 연결 응답, 창 생성 및 화면
출력은 별도 통합 시험으로 남아 있다.
