# Mosaic on the five-role AArch64 guest / 다섯 역할 게스트의 Mosaic

The original Xwayland server presented Mosaic through the native Wayland compositor on AArch64 QEMU virt. On the matching-credential boot, the initial window was coral. The first emulated tablet click changed the window to blue. The second click destroyed the window and Mosaic exited with status 0. **Xwayland then exited with status 1. The complete session failed its clean-shutdown criterion.** The observer did not record the five final exit records. [Matching-credential receipt](matching.json)

| Initial window / 최초 창 | After first pointer click / 첫 클릭 후 |
| --- | --- |
| ![Coral Mosaic cropped from the guest scanout](mosaic-coral.png) | ![Blue Mosaic cropped from the same guest after the first tablet click](mosaic-blue.png) |

Each image contains the observed 240 × 160 crop at (48, 56) in the 320 × 240 guest framebuffer. All 38,400 pixels matched the expected scene over two observations at least 200 ms apart. A separate cold boot of the same packaged kernel supplied a wrong client credential. Mosaic decoded the server's rejection and exited 114. Xwayland exited 0 in that boot. [Wrong-credential receipt](wrong.json)

A repeat with the same kernel reached dispatch and matched all 38,400 coral pixels again. Its first-click observation timed out waiting for blue, with no process exit recorded. The repeat also rejected the wrong credential with the same Mosaic and server exits. The first run demonstrates a real input transition. Reliable input and graceful shutdown remain open. A subsequent source change, integration `75d5e37`, orders Wayland role destruction and synchronizes button release with the observed blue frame. These receipts precede that change. [Repeat matching](repeat-matching.json) · [Repeat rejection](repeat-wrong.json)

동일한 AArch64 커널로 두 번 부팅했다. 잘못된 인증 자료를 제공한 부팅에서는 실제 서버가 연결을 거부해 Mosaic가 114, Xwayland가 0으로 종료했다. 정상 인증 부팅에서는 최초 산호색과 첫 가상 태블릿 클릭 후 파란색을 각각 38,400픽셀 전체에서 확인했다. 두 번째 클릭 뒤 Mosaic가 창 파괴 이벤트를 받고 0으로 종료했으나 Xwayland의 종료값은 1이었다. 따라서 전체 세션의 정상 종료 검증은 실패했다. 공개 소스에서의 독립 재현, 서버 재시작, 실보드 검증은 별도 항목이다.

재시도에서도 잘못된 인증 거부와 같은 산호색 화면을 확인했다. 정상 인증 재시도는 첫 클릭 후 파란 화면을 기다리다 시간 초과했고, 그때까지 기록된 프로세스 종료는 없었다. 후속 수정은 Wayland 역할 파괴 순서와 버튼을 떼는 시점을 조정한다. 이 측정은 수정 전 결과다.

### Build and image provenance / 빌드와 이미지 출처

The normal executable used objects built from integration revision `1e6b5850ccf2859cad5a37b6ff346708bbedeb32` and was relinked with the heap override preceding the linker script from `2a3ba8f6946014e0f6c2a4fc560712a2db601120`. The five-role native graphics adapter SHA-256 was `b4a91dcb62111bc9852b54a29db4bc2c0be17eaa5c176ed7e90cc47c8bde9901`. Its four-role parent has a separate build receipt. [Platform PR](https://github.com/Romanof123/JNOX/pull/473)

| Artifact | SHA-256 |
| --- | --- |
| Original Xwayland ELF | `4f39c60930f435e449c07273063c72e7df2155007790432be7146156275fc3a3` |
| Load-equivalent Xwayland ELF packaged in slot 1 | `09bd3d38d48a3f86d6b7eeb688180cf962946626360e6a8fee11e8d65bbfbb18` |
| Kernel used for both boots | `8901c89e5fc29f8dc00ce3ec97d65112ea5b9b3e8b43d89a47ed321e37a12ad2` |

The observer hashed each PPM at capture time. For publication, the retained PPM hash was checked against the receipt and its RGB bytes were losslessly encoded with Pillow. FFmpeg independently decoded the published PNG. All 115,200 decoded RGB bytes per frame matched the corresponding observed crop byte for byte. The PNGs contain only the image header, image data and end marker.

| Frame | Format | SHA-256 |
| --- | --- | --- |
| Coral | Original PPM | `8dad4a991fe4b5655b4b96d01db1fda83abb3abc2007611c40c8feb531eccd95` |
| Coral | Decoded RGB24 | `ad258a40711dd343a45f2f424db3e5e1637655060f3486b3740c39f65e578053` |
| Coral | Published PNG | `512e36745ba21dd86329b2e4b7e114abf115544da2ee7628742f733445069700` |
| Blue | Original PPM | `67ece280b7b957a1f71294793fab012f28b97dd56179befe0539cd5cba88c627` |
| Blue | Decoded RGB24 | `5fe0515348755371bc409b5d8bd546a3257bc157a4c7c5cad1a33d6f10f7d615` |
| Blue | Published PNG | `077f35a8a69f531a5dc7fe7c42c1ebd5316d1c7fe31df83af3361771a262e1f7` |

원본 PPM 해시를 게스트 관찰 영수증과 대조한 뒤 Pillow로 PNG를 만들었다. 별도 디코더인 FFmpeg로 PNG를 다시 읽어 각 화면의 RGB 115,200바이트가 원본과 일치함을 확인했다. 추적 항목은 CHG-340, REQ-015/024, DES-009/013, VER-015/024다. [Guest acceptance #448](https://github.com/Romanof123/JNOX/issues/448)
