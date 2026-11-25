# Native IPv4 socket service / 네이티브 IPv4 소켓 서비스

IPC-006 has separate local and network acceptance groups. This guide describes the IPv4 network group on single-core AArch64 QEMU virt with fixed-priority FIFO or RR. The isolated Rust process in `services/inet` owns transport state and uses smoltcp 0.12.0 with Ethernet, IPv4, TCP and UDP enabled. The kernel restricts access to the virtual interface to the selected broker slot. [IPC-006 #376](https://github.com/Romanof123/JNOX/issues/376) · [Local service](native-socket-service.en-ko.md)

IPC-006의 네트워크 그룹은 단일 코어 AArch64 QEMU virt의 FIFO·RR에서 IPv4를 사용한다. 격리된 일반 Rust 브로커가 전송 상태를 관리하며 커널은 지정한 브로커 슬롯에만 가상 인터페이스 권한을 준다. 로컬 AF_UNIX 결과와 네트워크 관측을 각각 기록한다.

## Package / 패키지

Give each client an explicit synchronous call route into the broker. Select the same slot for `--net-service` and `--socket-service`. Raw Cargo builds use matching `JNOX_NATIVE_NET_SERVICE` and `JNOX_NATIVE_SOCKET_SERVICE` values plus `JNOX_NATIVE_IPC_ROUTES`. The package manifest records the slot, transport, MAC and operation numbers. For the network broker, x8=38 receives a frame, 39 transmits a frame and 40 polls the broker's incoming IPC without blocking. Operation 40 allows the service to process network traffic while no client request is ready.

클라이언트마다 브로커로 향하는 동기 IPC 호출 경로를 명시한다. 두 서비스 옵션과 직접 Cargo 빌드의 두 환경 변수는 같은 슬롯이어야 한다. 매니페스트에는 슬롯, 전송 장치, MAC, 연산 번호가 기록된다. 브로커는 38번으로 프레임을 받고 39번으로 전송한다. 40번은 준비된 IPC 요청을 비차단 방식으로 확인한다.

For a C client in slot 0, a Rust client in slot 1 and the network broker in slot 2, set `C_ELF` and `RUST_ELF` to independently built IPv4 client ELFs configured for broker slot 2. The mask `3` designates clients 0 and 1. Pass workload-specific client arguments using `--arg` and `--peer-arg`. Use fresh output directories for each package.

슬롯 0·1은 각각 C·Rust IPv4 클라이언트, 슬롯 2는 네트워크 브로커다. 아래 두 환경 변수에 슬롯 2를 사용하는 클라이언트 ELF 경로를 설정한다. 클라이언트별 인자는 `--arg`와 `--peer-arg`로 전달한다.

~~~sh
sh sdk/native-aarch64/build-rust.sh services/inet/Cargo.toml inet-broker.elf
package() {
    policy=$1
    shift
    python3 tools/build_native_el0.py --board virt --policy "$policy" --out "inet-$policy" \
        --elf "${C_ELF:?set C_ELF}" --priority 7 --peer-elf "${RUST_ELF:?set RUST_ELF}" --peer-priority 4 \
        --peer-elf inet-broker.elf --peer-priority 1 --peer2-arg 3 \
        --socket-service 2 --net-service 2 --ipc-route 0:2 --ipc-route 1:2 "$@"
}
package fifo
package rr --quantum-ticks 625000
~~~

## Isolated Ethernet peer / 격리된 이더넷 상대

The guest uses 10.0.2.15/24 and MAC 52:54:00:12:34:56. The controlled peer and configured router address is 10.0.2.2. The declared workload uses guest TCP/UDP ports 43001/43003 and peer TCP/UDP ports 43002/43004. These are workload parameters. For an isolated Linux TAP peer, run the following in a fresh network namespace with iproute2, tcpdump, QEMU and the required network privileges. The namespace has only loopback and the TAP. The host TCP/UDP peer runs inside it.

게스트 주소는 10.0.2.15/24, 제어된 상대와 라우터 주소는 10.0.2.2다. 워크로드의 TCP·UDP 포트는 게스트 43001·43003, 상대 43002·43004다. 아래 Linux 예시는 새 네임스페이스에 루프백과 TAP만 구성한다. iproute2, tcpdump, QEMU 및 네트워크 관리자 권한이 필요하다.

~~~sh
sudo ip netns add jnox-inet
sudo ip -n jnox-inet link set lo up
sudo ip netns exec jnox-inet ip tuntap add dev jnox0 mode tap
sudo ip -n jnox-inet addr add 10.0.2.2/24 dev jnox0
sudo ip -n jnox-inet link set jnox0 up
~~~

Start the capture in one terminal before the guest boots. In another, start the independently controlled peer with `sudo ip netns exec jnox-inet` and its normal command. Give each position a fresh capture filename. Boot its matching kernel in a third terminal. Stop tcpdump with Ctrl-C after the peer and guest have finished so it flushes the pcap.

게스트 부팅 전에 첫 터미널에서 패킷 기록을 시작하고, 두 번째 터미널에서 같은 네임스페이스의 독립 상대를 실행한다. 세 번째 터미널에서 해당 커널을 부팅한다. FIFO와 RR에는 다른 pcap 파일을 쓰고, 실행 종료 후 tcpdump에서 Ctrl-C로 저장을 마친다.

~~~sh
sudo ip netns exec jnox-inet tcpdump -ni jnox0 -s 0 -U -w inet-fifo.peer.pcap
~~~

~~~sh
sudo ip netns exec jnox-inet qemu-system-aarch64 \
    -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg -m 128M \
    -nodefaults -display none -monitor none -no-reboot -serial stdio \
    -global virtio-mmio.force-legacy=false \
    -netdev tap,id=peer,ifname=jnox0,script=no,downscript=no \
    -device virtio-net-device,netdev=peer,mac=52:54:00:12:34:56 \
    -kernel inet-fifo/kernel.elf
~~~

Repeat with the RR package and a separate pcap. Retain the package manifests, client/broker/kernel hashes, QEMU command and version, UART, independent peer log and raw Ethernet captures. Verify actual TCP connection, application bytes and reset, then two UDP datagrams with distinct payloads and source addresses/ports as declared by the peer. Record the sent sequence, controlled omission/reordering and exact guest delivery order. Separate packets observed on the interface from bytes returned to each application. Approval requires independent evidence from both local and network groups. IPv6 and target-specific driver, DMA and coherence coverage remain under their respective profiles. [IPC tracker](https://github.com/Romanof123/JNOX/issues/24) · [Network coverage](https://github.com/Romanof123/JNOX/issues/8) · [Board qualification](https://github.com/Romanof123/JNOX/issues/34)

RR도 별도 pcap으로 반복한다. 매니페스트·실행 파일 해시·QEMU 명령 및 버전·UART·상대 로그·원본 패킷을 보존한다. TCP 연결과 실제 데이터·리셋, 서로 다른 UDP 데이터그램 두 개의 내용·상대 주소와 포트, 의도한 손실·재정렬 및 게스트 도착 순서를 대조한다. 인터페이스의 패킷과 앱이 반환받은 데이터를 구분한다. 두 그룹의 독립 근거가 있어야 승인할 수 있다. IPv6, 물리 장치 드라이버·DMA·캐시 일관성 검증은 해당 프로필에서 다룬다.

QEMU 6.2 filter-dump on the tested TAP path included a 10- or 12-byte virtual network header in records labelled Ethernet. Use the independent Linux interface capture for ordinary Ethernet decoding. The packet verifier retains both views and checks the framing per capture. / 해당 TAP 구성에서는 독립 인터페이스 캡처에서 Ethernet 프레임을 직접 읽을 수 있다. [Original run](evidence/native-ipv4-socket-first.en-ko.md) · [Corrected run and captures](evidence/native-ipv4-socket-corrected.en-ko.md).

**IMP-307 / EVD-307.** Independent Release rebuilt the published implementation and accepted the CPU-only virtual IPv4 group on FIFO and RR. The C TCP app distinguished EOF from ECONNRESET. UDP retained sender address and port for two senders, represented empty datagrams, and reported the original record length with MSG_TRUNC even when only three bytes fit. The local AF_UNIX backlog and finite-deadline case remains with #381. / 공개 소스를 재빌드한 결과 두 스케줄링 정책에서 TCP 정상 종료·리셋과 UDP 발신지·빈 메시지·잘림 처리를 확인했다. [Independent release / 독립 배포 판정](evidence/native-ipv4-socket-release.en-ko.md).
