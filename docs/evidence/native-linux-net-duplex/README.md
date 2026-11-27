# EVD-336 · Native Linux-source full-duplex NIC / 원본 네트워크 드라이버 양방향 검증

Both cold boots delivered 80 requests to a separate native application and returned its 80 distinct replies through the registered Linux TX callback. One final inbound acknowledgement brought the total to 161 Ethernet frames per boot. The first TX descriptor was still pending when the application received its second request. Both 32-entry rings wrapped twice. The original TX callback ran 80 times per boot. The provider reaped TX completions. TX completion interrupt callback execution and physical hardware remain unverified. [Change and requirements](../../native-linux-net-duplex.en-ko.md) · [Issue #441](https://github.com/Romanof123/JNOX/issues/441) · [PR #443](https://github.com/Romanof123/JNOX/pull/443)

서로 다른 MAC·MTU에서 두 번 부팅해 같은 네트워크 장치의 원본 송수신 경로를 확인했다. 매번 별도 앱이 만든 응답 80개가 호스트 소켓과 NIC 패킷 캡처에서 바이트 단위로 일치했다. 첫 송신이 장치에 통지되기 전에 두 번째 요청이 앱에 전달됐다. 수신과 송신 링은 각각 두 번 순환했다. 송신 완료는 제공자 코드가 회수했고, 송신 완료 인터럽트 콜백 실행 및 실보드 동작은 별도 검증 대상이다.

압축 파일을 빈 디렉터리에 풀고 `python3 -B replay.py`를 실행하면 파일 1,397개의 체크섬, 독립 판독 결과 두 건, 부팅당 손상 대조 시험 네 건을 확인할 수 있다. 원래 실행 파일 경로와 실행 인자를 보존했다. 두 시험 당시 QEMU 실행 파일 해시는 별도로 측정하지 않았으며 보드 기준선의 해시를 구분해 기록했다. [Archive / 검증 원본](duplex.tar.xz) · [SHA-256](duplex.tar.xz.sha256) · [Results / 실측값](result.json)

Extract into an empty Linux directory and run `python3 -B replay.py`. The offline replay uses Python 3.10 or newer, GNU nm and aarch64-linux-gnu-nm. Pyelftools 0.32 and its license are bundled.

| Boot | Build commit | Frozen observer | MAC | MTU |
|---|---|---|---|---|
| cold-01 | cb2ec7625818c3cfcbfb6d70aad2f5915a2a93fb | cb2ec7625818c3cfcbfb6d70aad2f5915a2a93fb | 52:54:00:44:10:01 | 1410 |
| cold-02 | cb2ec7625818c3cfcbfb6d70aad2f5915a2a93fb | cb2ec7625818c3cfcbfb6d70aad2f5915a2a93fb | 52:54:00:44:10:52 | 1490 |

Each capture retains six stopped checkpoints, the protected RX grant table, RX/TX ring and DMA contents, two private address spaces, trace byte offsets, raw length-prefixed ingress and egress streams, Ethernet PCAP, UART, QEMU execution and device traces, and the exact launch command. The held and overlap stops permit checking RX progress while the first submitted TX descriptor is still unnotified. The launch records identify the executable path. They omit a capture-time QEMU executable digest. The board's separately pinned executable digest is identified in the report.

The archive retains the original capture stdout under observations/, the captured kernel and both embedded role ELFs, object files, maps, and the pinned original Linux translation unit. Each build's source index maps every recorded compiler dependency to archived bytes. The header tree digest ledger covers all 9,403 pinned files and the compiled subset is present. Replay compares the original stdout and all four corruption outcomes to the frozen observer result. It separately recomputes the independent reader's retained JSON, reads both 80-slot histories and the final ACK directly from the peer's translated RAM, and compares them with the two external streams. In each checkpoint summary the four indices are RX available, RX used, TX available and TX used. The final indices summarize completed traffic; terminal release is checked from scrubbed rings.

Individual offline commands from the extracted directory follow.

```sh
PYTHONPATH=vendor python3 -B observers/cb2ec7625818c3cfcbfb6d70aad2f5915a2a93fb/tools/verify_linux_source_net_duplex.py --replay --mutation-tests --package builds/8ba4bd5b0d97eefdba9c96e197f834674f8b7f5e535cf5722149e18acfcbad89/native --driver-manifest builds/8ba4bd5b0d97eefdba9c96e197f834674f8b7f5e535cf5722149e18acfcbad89/imported/import.json --out captures/cold-01 --mac 52:54:00:44:10:01 --mtu 1410
PYTHONPATH=vendor python3 -B qa/replay.py builds/8ba4bd5b0d97eefdba9c96e197f834674f8b7f5e535cf5722149e18acfcbad89 captures/cold-01 source/linux/4700e3adf3b24b39dd4929e9c6d62811ba3ccea8/drivers/net/virtio_net.c
PYTHONPATH=vendor python3 -B observers/cb2ec7625818c3cfcbfb6d70aad2f5915a2a93fb/tools/verify_linux_source_net_duplex.py --replay --mutation-tests --package builds/8ba4bd5b0d97eefdba9c96e197f834674f8b7f5e535cf5722149e18acfcbad89/native --driver-manifest builds/8ba4bd5b0d97eefdba9c96e197f834674f8b7f5e535cf5722149e18acfcbad89/imported/import.json --out captures/cold-02 --mac 52:54:00:44:10:52 --mtu 1490
PYTHONPATH=vendor python3 -B qa/replay.py builds/8ba4bd5b0d97eefdba9c96e197f834674f8b7f5e535cf5722149e18acfcbad89 captures/cold-02 source/linux/4700e3adf3b24b39dd4929e9c6d62811ba3ccea8/drivers/net/virtio_net.c
```
