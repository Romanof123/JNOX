# Original Linux network driver baseline / Linux 네트워크 드라이버 원본 기준 시험

A single AArch64 Linux guest booted on QEMU virt and loaded the unmodified Alpine `virtio_net.ko` into Linux `6.18.65-0-virt`. Linux sysfs identified the NIC driver as `virtio_net` and the transport as `virtio-mmio`. The guest received a random 32-byte host challenge encoded as 64 hexadecimal characters and returned the identical text over the NIC. The host also fetched the device tree over that interface and matched its SHA-256 to the guest's own measurement. Offline replay passed.

AArch64 Linux를 부팅해 원본 모듈과 의존 모듈 3개를 그대로 로드했다. 실제 장치 연결을 확인하고 호스트가 보낸 32바이트 난수의 16진수 표현이 네트워크를 통해 그대로 돌아오는지 대조했다. 장치 트리도 같은 인터페이스로 읽어 게스트에서 계산한 해시와 대조했다.

| Artifact / 산출물 | Identity / 기준값 |
| --- | --- |
| Stock module loaded inside Linux | `3873c32ab124c0a83e7a7f04c27fde93b230c45236bb9fc0b9125abee19dca4f` |
| Decompressed AArch64 kernel Image | `4eb5c87867473f20f5ce5084c23f3dff587c84327f3fbba0cfe3157133c20c29` |
| Kernel configuration | `72f421144719596a38b40adb2eec8ded2628827518438d23d5b3f0f97cee02ed` |
| Guest-visible device tree from this boot | `732d2a8232fa6ebd8ff05eaa1757e84f9065a85c1c04c62ef2be8523abaedfd3` |

[Exact request, response and observed driver binding](observations.txt) · [Machine-readable capture results](result.json) · [Source, package signatures and module provenance](INTAKE.md) · [Package and source file hashes](provenance.json) · [Original download locations](downloads.json).

To reproduce the Linux-only test, fetch the two named APKs from the pinned distribution paths and check their hashes from `result.json`. Place them in one directory. Run from a Linux checkout with Python 3 and QEMU for AArch64. The observer builds the initramfs, boots the guest and binds the network forward to loopback. For the separate offline signature and source checks, retrieve the inputs in `downloads.json` alongside a copy of the intake verifier. A mutable distribution index may require the retained signed snapshot.

```sh
python3 -B tools/verify_linux_virtio_net_baseline.py --intake /path/to/pinned-apks --out linux-alone
python3 -B tools/verify_linux_virtio_net_baseline.py --replay linux-alone
```

**EVD-321** ties the Linux-only result to REQ-004, DES-002, IMP-321 and VER-004. [Frozen observer source](https://github.com/Romanof123/JNOX/commit/81ed3df16a00a27950db0daedc1aba1504acb1b2) · [Design](../../linux-virtio-net-baseline.en-ko.md) · [Baseline #416](https://github.com/Romanof123/JNOX/issues/416). The original Linux-only capture is retained locally. Compact packet SHA-256 `848d1677ee5135c4dae509b42e78e0c5c8d5c9e73647d8eee1919b6bcb510f96`. The compressed kernel, initramfs and original signed packages can be identified separately from these records.

Next compare the identical Linux module with requests originating inside JNOX, then test bounded fallback, reset, memory/interrupt/DMA isolation and release timing. This baseline used a Linux-only guest and QEMU user networking on loopback. / 다음 단계에서는 같은 모듈을 사용해 JNOX 앱의 요청을 전달하고 Linux 중단과 재연결을 검증한다. 단독 가상 보드 시험은 물리 장치 격리나 제품 안전 승인을 포함하지 않는다. [Carryover integration #3](https://github.com/Romanof123/JNOX/issues/3)
