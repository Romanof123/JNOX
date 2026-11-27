# Registered TX completion on QEMU virt

## Design and implementation

The opt-in #456 workload uses the same registered Linux netdev, virtqueue pair, peer, and wire protocol as #438. Primary argument `t` selects TX interrupt completion. Each submitted frame arms queue 1 before publishing its descriptor. For this bounded profile the Linux module parameter napi_tx is false and one TX frame may be outstanding. The registered upstream skb_xmit_done disables further queue interrupts and calls netif_wake_subqueue. The owner worker then retrieves the returned descriptor cookie and releases its skb. The worker samples the protected receipt while waiting and sleeps between attempts. RX retains its existing blocking IRQ and NAPI path.

On real SPI 79 delivery EL1 reads and acknowledges the virtio-mmio interrupt status. For a used-buffer indication with the TX ring armed and notifications enabled, it checks the device-written used index, returned descriptor ID, zero used length and current outstanding ownership. It records a token-bound receipt only once. SVC 169 mode 3 arms an idle queue and mode 2 claims a receipt. A claim with no receipt returns EAGAIN. A retired token returns ESTALE. Removing the device cancels pending delivery. Both queues share the interrupt line. The IRQ can cover RX and TX together, and the protected journal records concurrent RX progress. A TX-only interrupt source and a direct interrupt-to-EL0 dispatch latency are outside the claim.

## Acceptance cases

1. Build the ordinary registered-duplex service from the pinned virtio_net.c with the matching prepared Linux headers. Boot once without a primary argument for the existing polling regression.
2. Build a second package with primary argument t. Keep the peer, transport and IRQ route identical. In the held and overlap snapshots, a deferred TX descriptor is present, the TX used index is unchanged, and the first IRQ receipt and callback rows are empty even after an RX delivery.
3. Run the existing 80 externally challenged duplex exchanges. For each reply, match socket/PCAP bytes, the hardware used entry, the EL1 event and the independently traced registered skb_xmit_done instruction address. Check the callback lies after an actual SPI 79 IAR/status/ACK/EOI interval and before the next TX submission. The zero-weight callback enters the Linux queue wake adapter exactly once; the worker rejects a duplicate receipt and releases exactly one matching cookie.
4. Verify the peer receives EACCES when claiming the current owner's receipt, an empty claim returns EAGAIN and a retired token returns ESTALE. Replay rejects mutations to protected IRQ records or the callback journal. Repeat with another MAC/MTU and retain the original unmodified duplex regression.

```sh
python3 tools/build_linux_source_virtio_net.py --source "$source" --headers "$headers" --out "$import" --lifecycle-duplex-service
python3 tools/build_native_el0.py --board virt --policy fifo --linux-source-net-duplex --elf "$import/service.elf" --peer-elf "$import/peer.elf" --arg t --out "$package"
python3 tools/verify_linux_source_net_duplex.py --package "$package" --driver-manifest "$import/import.json" --out "$capture" --require-tx-irq --mutation-tests
python3 tools/verify_linux_source_net_duplex.py --package "$package" --driver-manifest "$import/import.json" --out "$capture" --require-tx-irq --replay
```

## 설계와 검증

주 프로세스 인자 t로 기존 양방향 네트워크 시험의 TX 인터럽트 경로를 선택한다. 인자를 생략하면 기존 폴링 경로를 유지한다. 송신 큐에 한 프레임만 게시하며 원본 리눅스 드라이버가 등록한 skb_xmit_done을 호출한다. 이 구성은 TX NAPI 가중치가 0이므로 콜백에서 알림을 끄고 송신 하위 큐를 깨운다. 실제로 반환된 디스크립터의 쿠키를 확인하고 skb를 해제하는 작업은 소유자 워커가 수행한다.

EL1은 실제 SPI 79에서 장치 인터럽트 상태, 사용 완료 인덱스, 디스크립터 ID, 반환 길이와 현재 소유권을 확인한다. 수신과 송신이 같은 인터럽트 선을 사용하므로 하나의 인터럽트에서 양쪽 진전이 함께 관측될 수 있다. 외부 요청을 처리하면서 첫 송신 킥을 보류한 두 시점에는 TX 콜백이 없어야 한다. 80회 교환 각각에서 외부 패킷, 장치 링, EL1 기록, 원본 콜백 실행 주소, 하위 큐 깨우기와 쿠키 해제를 대조한다. 권한이 없는 피어, 중복 수령, 장치 제거 후 옛 토큰의 거부도 확인한다. 다른 MAC·MTU로 재실행하고 기존 폴링 방식의 회귀 결과와 함께 보관한다. 이 문서는 설계와 실행 조건을 기술한다. 게스트 실측과 독립 검증은 별도 단계다.
