# Offline native virtio-net TX evidence

Use Linux with Python 3.10+ and GNU aarch64-linux-gnu-nm. Extract this ZIP into an empty directory, enter it and run the following commands. The vendored pyelftools 0.32 and its license are included. All invocations use frozen captures and the replay flag.

```sh
export PYTHONPATH="$PWD/vendor"
export PYTHONDONTWRITEBYTECODE=1
build=456-build-01
verify=producer/corrected/verify_linux_source_net_duplex.py
for run in tx-first-v2 tx-repeat-v2; do
  python3 "$verify" --package "$build/tx" --driver-manifest "$build/imported/import.json" --out "$build/$run" --require-tx-irq --replay --mutation-tests || exit 1
done
python3 "$verify" --package "$build/ordinary" --driver-manifest "$build/imported/import.json" --out "$build/ordinary-v2" --replay --mutation-tests
```

The package retains the initial priority rejection (gate.log), initial observer failure (resume-01.log and tx-first-capture), final producer results (resume-02.log), launch argv, QEMU traces and PCAP, all stopped-machine checkpoint ranges, both native kernels, the imported original source objects, the linked service and peer. identity/tools.json records the QEMU version and executable hash read after capture, input source/header identities and exact producer verifier revisions. The full header APK was hashed separately. The two recorded generated headers and pinned upstream virtio_net.c are included. The first reader is preserved under producer/original. Supplied command scripts show the original machine-specific live invocations for provenance.
