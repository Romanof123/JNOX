# CHG-336 · Traffic in both directions on a registered NIC / 원본 드라이버 양방향 통신

REQ-004/011 · DES-002 · IMP-336. The AArch64 virt profile uses the netdev created by the original Linux probe. The RX queue dispatches the original NAPI path. The TX adapter calls the registered `ndo_start_xmit` on that same live netdev and retains the SKB until the device reports completion. Its task shares CPU0 with the ordinary application. Device configuration, guest DMA windows and interrupt-controller access are authorized by the Rust kernel. [Parent requirement #426](https://github.com/Romanof123/JNOX/issues/426) · [Acceptance #441](https://github.com/Romanof123/JNOX/issues/441)

원본 probe가 만든 실제 네트워크 장치 하나에서 양방향으로 통신한다. 수신은 원본 NAPI로 처리하고, 송신은 같은 객체에 등록된 콜백으로 처리한다. 장치가 완료한 후에만 송신 SKB를 반납한다. 애플리케이션은 파이프를 통해 송수신하며 장치 권한은 드라이버 프로세스에 있다.

## End-to-end test / 종단 시험

An external host supplies a new nonce and 80 sequenced Ethernet requests. The app receives exact request bytes over IPC, builds replies with interchanged addresses, a different EtherType and a payload transform bound to the nonce and sequence. The NIC must emit the replies on the emulated wire. Both 32-entry ring indices must wrap twice. The first TX is deliberately published with the driver's ordinary `xmit_more` batching active. The device notification is deferred while the unrelated second request is delivered to the application. A stopped observation at that boundary must show simultaneous TX ownership and live RX, followed by an actual TX kick, used return and release. [Fixture and peer](../examples/linux-source-net-duplex) · [Native capture](../tools/verify_linux_source_net_duplex.py)

호스트가 매 부팅마다 새 nonce로 요청 80개를 보낸다. 별도 앱이 주소·유형·내용이 달라진 응답을 만들고 실제 NIC를 통해 호스트로 돌려보낸다. 송수신 링의 두 번 회전과 실제 송신 완료를 확인한다. 첫 번째 응답이 송신 큐에 게시된 상태에서 두 번째 요청을 수신하는 구간을 정지 관측해 두 큐의 동시 활성 여부를 판단한다.

The observer must compare external socket and PCAP data with private application memory, linked callback-table entries and executed instruction addresses. Confirm peer device-access denials, reset/cleanup, and clean exit under two MAC/MTU configurations. Record guest evidence and independent review separately when available. [Trace graph](../trace/links.csv)

서로 다른 MAC·MTU에서 두 번 부팅하고, 외부 패킷·소비자 메모리·원본 콜백 포인터·장치 큐 상태를 연결한다. 변경 설계 시점에는 게스트 실행과 독립 검증이 남아 있다.

EVD-336 records both cold boots and the independent reconstruction of all requests, replies, queue wraps, overlapping RX and TX, and checked device release. 두 부팅에서 소비자 메모리와 외부 송수신 기록을 대조했다. [Results and raw replay / 결과와 재현 자료](evidence/native-linux-net-duplex/README.md)

The combined source subsequently passed ordinary lifecycle, provider failure, service assertion and duplex regressions. 동일 소스에서 네 구성을 각각 부팅했다. [Integrated evidence / 통합 회귀](evidence/native-linux-net-integrated/README.md)
