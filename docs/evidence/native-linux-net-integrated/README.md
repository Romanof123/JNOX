# Four native VirtIO-net profiles on one combined revision / 원본 드라이버 통합 회귀

REQ-004/011 · EVD-334/335/336. [Driver lifecycle](../native-linux-net-lifecycle) · [Fault containment](../native-linux-net-assertion) · [Two-direction traffic](../native-linux-net-duplex) · [Integration issue #426](https://github.com/Romanof123/JNOX/issues/426)

같은 JNOX 소스에서 네 가지 설정을 각각 빌드해 한 번씩 부팅했다. 정상 수명주기, 네 곳의 초기화 실패와 복구, 큐 활성 중 드라이버 단언과 기존 소비자 생존, 같은 장치의 원본 양방향 통신을 확인했다. 정지 관측 31회와 손상 대조 시험 24건을 포함한다. 빈 디렉터리에서 `python3 -B replay.py`로 원본 1,610개 파일과 네 가지 판독 결과를 재현할 수 있다. 실제 게스트 실행 파일의 해시는 고장 주입·단언·양방향 시험에서 실행 전후 및 실행 중 측정했다. 정상 시험의 외부 측정은 실행 후에 이루어졌고 관측기의 시작 시점 해시를 함께 기록했다. 이 묶음은 동일 코드의 통합 회귀 기록이며, 각 기능의 독립 판독 원본은 위 세 문서에서 확인할 수 있다. [Archive / 원본](integrated.tar.xz) · [SHA-256](integrated.tar.xz.sha256) · [Results / 실측](result.json)

Guest source and frozen primary observers were both built or exported at JNOX Git commit `fcef15921d0d8cc3a0eb71e8b77273ba07990dc4`. Four separate AArch64 virt/FIFO kernels each cold-booted their own driver and peer image. From a new extraction on Linux, run `python3 -B replay.py`. Python 3.10+, GNU nm and aarch64-linux-gnu-nm are required. Pyelftools 0.32 is bundled. This command verifies every archived file, decodes all four observed boots from their original RAM and traces, and compares all mutation verdicts to the original observer stdout.

| Profile | Stopped checkpoints | Corruption controls | External QEMU executable measurement |
|---|---:|---:|---|
| ordinary | 8 | 4 | After run. Observer also recorded launch binary SHA-256. |
| failures | 15 | 4 | Before, live process and after. |
| assertion | 2 | 12 | Before, live process and after. |
| duplex | 6 | 4 | Before, live process and after. |

**Evidence boundaries.** One complete two-generation ordinary lifecycle, four controlled EL0 provider failures followed by clean recovery, one deliberate owner BRK with peer survival, and one full-duplex RX/TX lifecycle ran on the same source revision. These are four independent cold boots. Single-profile acceptance packages contain the separate independent reader results on their earlier captures. This archive reproduces the capture-time primary observer on the new integrated specimens.

The profile directories retain the original capture stdout, streams, PCAP, UART, QEMU execution/IRQ/MMIO trace, QMP stops and sparse physical reads, including page tables used by the observers. Duplex checkpoints also retain QEMU trace byte offsets. The assertion capture retains its last scratch probe separately within the original capture directory. No hardware TLB tracing or physical-device measurement is claimed.

Each launch.json carries the observed QEMU command line. For the three externally monitored boots, the process log records the live PID, executable target, SHA-256 via /proc/PID/exe, and the actual process argument list. The surrounding records contain executable path, version and SHA-256. Ordinary ran before the external process recorder started. Its launch digest and after-run record are checked separately. The source index under each build maps every compiler input to archived bytes. The original upstream Linux C file, actual header dependencies, complete pinned header hash ledger, linked maps, objects, native ELFs, import manifests, compiler version identities and command lines are included. The common source and headers are stored once.

provenance/combined-commands.sh transcribes the acquisition owner's build and capture command. The retained profile stdout, build logs and launch records are original measurements. Individual offline observer commands follow.

```sh
PYTHONPATH=vendor python3 -B observer/tools/verify_linux_source_net_lifecycle.py --replay --mutation-tests --package builds/2e6449e0a2517e9c5465fc8b0aff5d5dfd765d1c651597bf6238e8a5e38c4a33/native --driver-manifest builds/2e6449e0a2517e9c5465fc8b0aff5d5dfd765d1c651597bf6238e8a5e38c4a33/imported/import.json --out profiles/ordinary/cold-01 --mac 52:54:00:19:26:41 --mtu 1410
PYTHONPATH=vendor python3 -B observer/tools/verify_linux_source_net_lifecycle_failures.py --replay --mutation-tests --package builds/a4147c9810a3a94a0a87c8f5874dc4907d4e8face65d6a1048956277f32bb6dc/native --driver-manifest builds/a4147c9810a3a94a0a87c8f5874dc4907d4e8face65d6a1048956277f32bb6dc/imported/import.json --out profiles/failures/cold-01 --mac 52:54:00:19:26:41 --mtu 1410
PYTHONPATH=vendor python3 -B observer/tools/verify_linux_source_net_assertion.py --replay --mutation-tests --package builds/3d2c60b2caa21b583c34e5a0b1a37fa1960e9d43a5d4b19d0b776d5a5e59aa71/native --driver-manifest builds/3d2c60b2caa21b583c34e5a0b1a37fa1960e9d43a5d4b19d0b776d5a5e59aa71/imported/import.json --out profiles/assertion/cold-01 --mac 52:54:00:19:26:41 --mtu 1410
PYTHONPATH=vendor python3 -B observer/tools/verify_linux_source_net_duplex.py --replay --mutation-tests --package builds/a3819c6a7aab87308e7deb5a250ce213bdfe4b1c79b2f4a0c128590d14d146ac/native --driver-manifest builds/a3819c6a7aab87308e7deb5a250ce213bdfe4b1c79b2f4a0c128590d14d146ac/imported/import.json --out profiles/duplex/cold-01 --mac 52:54:00:44:10:01 --mtu 1410
```
