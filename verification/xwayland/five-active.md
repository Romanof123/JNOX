# Five active display processes (#465, #448)

Use the [resident Xwayland and Mosaic procedure / 실제 서버 실행 절차](resident/README.en-ko.md) for a linked executable. The earlier five-slot examples below describe the component topology.

The opt-in virt FIFO package starts all five executable images. Slot 0 owns the framebuffer, PL011 input and optional tablet. Xwayland (1) and the second native client (2) connect to its Wayland listener. Mosaic (4) connects to Xwayland's X11 listener through the AF_UNIX broker (3).

Build the broker with the matching feature. Supply the application's required arguments, environment and private Xauthority provider when integrating the real binaries.

```sh
sh sdk/native-aarch64/build-rust.sh --features display-five-active services/socket/Cargo.toml "$out/broker.elf"
python3 tools/build_native_el0.py --board virt --policy fifo --display-five-active \
  --elf "$compositor" --priority 7 --peer-elf "$xwayland" --peer-priority 5 \
  --peer-elf "$native_client" --peer-priority 5 --peer-elf "$out/broker.elf" --peer-priority 1 \
  --peer-elf "$mosaic" --peer-priority 5 --display-service 0 --socket-service 3 --shared-fd \
  --pointer-tablet --ipc-route 0:3 --ipc-route 1:3 --ipc-route 2:3 --ipc-route 4:3 --out "$out/package"
```

The package inserts decimal client mask 23 in the broker's argv. Kernel route validation admits only those four caller-to-broker edges. The fixed scheduler uses its ordinary startup constructor. Only display standby uses the dormant-slot handoff. All five tasks have their own saved frame and user address space. The authenticated caller selects the broker's per-process table, with four socket descriptors (64–67) per process, twelve global socket objects, two pending connections per listener and eight shared-right attachments in flight. The poll encoding remains four descriptors wide. Slots 0, 1 and 2 retain the shared descriptor API. Slot 4 cannot create, map, send or receive a managed shared FD. Optional console writing grants output on fd 1/2 separately.

The socket engine's feature-specific test opens both listeners, connects all three streams concurrently, checks the bytes at their respective peers and closes slot 4. Run the tests separately with the default and five-active configurations.

```sh
rustc --edition 2021 --test services/socket/src/local.rs -o "$out/socket-default" && "$out/socket-default"
rustc --edition 2021 --test --cfg 'feature="display-five-active"' services/socket/src/local.rs -o "$out/socket-five" && "$out/socket-five"
```

For integration, reject a missing 4:3 route, an extra 4:0 route, a broker mask of 7, and a package selecting both five-slot modes. Check default four-role and standby packages using the original broker. In the active guest verify both protocol endpoints, independent clients and the slot-4 denials at the syscall boundary. Runtime results require a separate gate.

## 한국어

이 프로필은 다섯 ELF를 일반 FIFO 태스크로 시작한다. 슬롯 0만 화면과 원시 입력에 접근한다. 슬롯 1의 Xwayland와 슬롯 2의 네이티브 클라이언트가 Wayland 소켓에 접속하고, 슬롯 4의 Mosaic은 슬롯 3의 브로커를 통해 슬롯 1의 X11 소켓에 접속한다. 브로커가 커널에서 확인한 호출자 번호로 파일 서술자 표를 선택한다. 슬롯 4에는 공유 메모리 생성·매핑·권한 전송을 허용하지 않는다.

검증할 때에는 두 리스너와 세 스트림을 동시에 유지하고 각 수신자가 받은 데이터를 확인한다. 잘못된 IPC 경로, 브로커 마스크 및 두 프로필의 동시 선택을 거부해야 한다. 기존 네 프로세스 모드와 대기 프로세스 복구 모드는 기본 브로커로 별도 확인한다. 이 문서는 실행 절차와 설계를 기록한다.

## Build the integrated server and Mosaic

From the checkout root, set the staged dependency paths in server-executable.md
and supply the two pinned host packages identified in selected-xkm.md. Run one
build at a time.

```sh
JNOX_NATIVE_XAUTHORITY=1 bash verification/xwayland/server-lab.sh "$SERVER_OUT" HEAD \
  --selected-xkm services/wayland/src/input.rs "$XKBCOMP_DEB" "$LIBXKBFILE_DEB"
sh sdk/native-aarch64/build.sh --local-socket examples/native-x11/client.c "$MOSAIC_ELF"
python3 examples/native-x11/local_profile.py prepare "$PRIVATE_DIR" --node "$NODE"
```

For the five-role package select `--nodename "$NODE"`, pass `:0`, `-auth`, and
`/run/Xauthority` as individual slot-1 arguments and `unix` as the only slot-4
argument. Supply the Wayland socket environment for slot 1. Provide private
role files to QEMU as described in docs/native-display-authority.en-ko.md.
The build should report a missing platform API if any upstream dependency is
incomplete. The latest isolated libX11 compile reached Xtrans FIONREAD. Full
server linking, startup and five-role rendering require separate gate results.

같은 체크아웃에서 선택 키맵과 필수 X 인증을 함께 활성화해 서버를 빌드한다.
노드 이름과 디스플레이가 일치하는 역할별 비공개 파일을 사용한다. 위 명령은
빌드 절차이며, 다섯 프로세스의 실제 화면 출력과 인증 거부는 게스트에서 검증한다.
