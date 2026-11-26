# Reproduce the Linux driver gateway / Linux 드라이버 게이트웨이 재현

Use a Linux checkout of the PR, Python 3, the repository’s native AArch64 Rust toolchain, an AArch64 static C cross-compiler and `qemu-system-aarch64`. The Linux input directory must contain the pinned `linux-virt-6.18.65-r0.apk` and `busybox-static-1.37.0-r30.apk`. The tracked Linux baseline result must be present for replay. [Pinned packages and provenance](../../docs/evidence/linux-virtio-net/INTAKE.md) · [Measured results](../../docs/evidence/linux-driver-gateway/README.md)

아래 명령은 저장소 루트에서 실행한다. `intake`는 기준 APK 두 개가 있는 경로로 지정한다. 세 JNOX 프로세스 중 첫 앱에만 네트워크 브로커 경로를 부여한다. Linux와 JNOX는 서로 다른 QEMU 프로세스에서 실행되며 Linux NIC 패킷은 게이트웨이와 별도로 기록한다.

```sh
set -eu
b=$(mktemp -d)
intake=/path/to/pinned-apks
export CARGO_BUILD_JOBS=1
python3 -B examples/linux-driver-gateway/linux/prepare.py --intake "$intake" --out "$b/linux"
for app in app heartbeat
do
    sh sdk/native-aarch64/build-rust.sh --local-socket \
        "examples/linux-driver-gateway/$app/Cargo.toml" "$b/$app.elf"
done
sh sdk/native-aarch64/build-rust.sh services/inet/Cargo.toml "$b/inet.elf"
python3 -B tools/build_native_el0.py --board virt --policy fifo \
    --elf "$b/app.elf" --priority 4 \
    --peer-elf "$b/heartbeat.elf" --peer-priority 5 \
    --peer-elf "$b/inet.elf" --peer-priority 7 --peer2-arg 1 \
    --ipc-route 0:2 --socket-service 2 --net-service 2 --out "$b/native"
python3 -B tools/verify_linux_driver_gateway.py --package "$b/native" \
    --linux "$b/linux" --out "$b/capture" --timeout 90
python3 -B tools/verify_linux_driver_gateway.py --replay "$b/capture"
python3 -B tools/test_linux_gateway_evidence.py --capture "$b/capture"
```

The public timing record identifies the exact guest runtime and each observer. To replay the retained archive, extract it into an empty directory. Use `--replay` on `first/capture` with the first observer and on `repeat/capture` with the repeat observer. Run the nine capture controls on `repeat/capture`. [Fixture design and wire contract](../../docs/linux-driver-gateway.en-ko.md)
