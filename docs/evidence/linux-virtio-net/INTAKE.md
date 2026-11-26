# AArch64 Linux network driver intake / AArch64 Linux 네트워크 드라이버 기준선

The Alpine v3.23 AArch64 linux-virt 6.18.65-r0 package supplies the kernel, configuration and loadable network driver. The matching signed repository index also identifies AArch64 busybox-static 1.37.0-r30. Both APKs are preserved intact. The direct-boot Image is the losslessly decompressed EFI zboot gzip payload. The four module binaries are the losslessly decompressed originals. The module metadata has no external firmware filename. `provenance.json` records each original and extracted hash, kernel configuration, modules.dep and vermagic. The same pinned artifacts should be used for stand-alone and integrated device runs.

두 패키지 및 저장소 인덱스에 포함된 원본 서명과 해시를 확인했다. 로컬 모듈 빌드·서명 변경은 없었다. Linux 단독 장치 동작, JNOX 게이트웨이를 통한 동작, Linux 정지·재시작 중 제어 앱 반응은 별도 검증 항목이다.

## Recheck the preserved files / 보존 원본 재검증

In a Linux work directory containing the inputs listed in downloads.json and a copy of verify_intake.py, use Python 3, OpenSSL, GNU readelf and patch to run

```sh
python3 -B verify_intake.py
sha256sum provenance.json
readelf --string-dump=.modinfo artifacts/virtio_net.ko
```

The verifier authenticates the signed APKINDEX and both packages using the public key obtained from the official Alpine HTTPS key service. APK v2 uses RSA/SHA-1 for the metadata signature and SHA-256 for the data tar member. The signed index also contains the hash of each compressed control member. The selected module's member and losslessly extracted ELF are checked separately. All parsing occurs offline after collection. The upstream driver file was checked against the stable patch and the seven Alpine patches named by the packaging recipe. This check identifies the source file and the distributed module. Reproduction of a compiler build is a separate activity. [Alpine public key](https://alpinelinux.org/keys/alpine-devel@lists.alpinelinux.org-616ae350.rsa.pub) · [Alpine packaging recipe](https://gitlab.alpinelinux.org/alpine/aports/-/blob/eb378b0757f36fcd639876eacb9315ef2cb9c075/main/linux-lts/APKBUILD) · [Linux source](https://github.com/gregkh/linux/blob/4700e3adf3b24b39dd4929e9c6d62811ba3ccea8/drivers/net/virtio_net.c)

The local `downloads.json` records every original URL, size and SHA-256. A CDN may retire a package or rotate its index. Retain the downloaded signed index and original APKs for independent replay.
