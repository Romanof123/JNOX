# Five active display processes (#465, #448)

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
