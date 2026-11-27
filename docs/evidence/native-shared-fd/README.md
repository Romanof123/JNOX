# JNOX #444 shared file descriptors and mapped frames

EVD-337 · REQ-024 · DES-013 · IMP-337 · VER-024. The producer closed and reused its local file descriptor. The receiver mapped the original 75 physical pages with read-only permission, then closed its installed descriptor. Its mapping stayed live while the producer updated the image. At terminal exit the kernel retained zero shared descriptors, mappings, escrow references and user page-table entries for those pages. [Change #444](https://github.com/Romanof123/JNOX/issues/444) · [PR #447](https://github.com/Romanof123/JNOX/pull/447) · [Design](../../native-shared-fd.en-ko.md) · [Measured results / 실측 결과](results.en-ko.md) · [Archive](jnox-444-shared-fd-a1876bd.tar.xz) · [SHA-256](jnox-444-shared-fd-a1876bd.tar.xz.sha256)

| Before producer update / 갱신 전 | After producer update / 갱신 후 |
|---|---|
| ![QEMU scanout before the update](before.png) | ![QEMU scanout after the update](after.png) |

These PNGs preserve the RGB pixels of the first cold boot's QMP framebuffer dumps. 두 그림은 첫 번째 부팅에서 QEMU가 저장한 화면의 픽셀과 일치한다.

## Cancellation at the broker / 브로커에서 대기 중인 호출의 취소

A separate reader decoded the frozen kernel and broker memory in both boots. At the first stop the kernel had delivered client 0's COMMIT token 57 to broker 3. Its pending receive requested one byte on a stream with zero queued bytes. After expiry the client recorded −110 and FD 98 was free. A later transfer used token 72 and installed the original object read-only at FD 98. Each capture rejected alterations to the pending token, caller slot and request type. The three observations bound the transition in time. [Reader](cancellation/cancel.py) · [Cold boot 1](cancellation/cold-1.json) · [Cold boot 2](cancellation/cold-2.json)

커널이 호출을 브로커에 전달했고 브로커가 같은 토큰으로 빈 스트림에서 대기했음을 양쪽 메모리에서 독립 확인했다. 만료 후 FD는 비어 있었고, 이후 정상 전송은 다른 토큰으로 수신자에게 설치됐다. To reproduce, extract the packet to `extracted`, run `python3 -B extracted/replay.py --restore "$PWD/restored"`, then run `python3 cancellation/cancel.py extracted/build restored/cold-1` and repeat with `cold-2` from the directory containing this README.

Two cold QEMU AArch64 boots exercised descriptor transfer between native processes using the local socket broker. In each boot, six stopped 128 MiB physical memory images cover the receiver before timeout, the queued descriptor, the installed descriptor, the two scanouts and terminal cleanup. One additional cold boot exercised the existing four-process window demo. Source revision a1876bd4dc51e292fe63b0785bc4ea586b367b97.

## Replay

Use Linux, Python 3.10 or later, GNU nm (binutils) and a writable temporary filesystem. The Python dependency used by the window observer is bundled. The replay runs without a guest, compiler, network or system package installation when those prerequisites are present. From a new directory extract the archive and run:

```sh
sha256sum -c jnox-444-shared-fd-a1876bd.tar.xz.sha256
mkdir extracted
tar -xJf jnox-444-shared-fd-a1876bd.tar.xz -C extracted
python3 -B extracted/replay.py
```

Use `python3 -B extracted/replay.py --restore /new/absolute/path` to retain a fully reconstructed, conventionally named FD capture directory. The path must be new and outside the extracted bundle. The mutation runner uses temporary copies on a Linux filesystem that supports symlinks. Allow at least 2 GiB free for filesystems that allocate sparse files in full.

## Captured evidence and checks

- Each full 134,217,728-byte physical image is indexed in `ram-layout.json`. Absent 64 KiB blocks contain zeros. Nonzero blocks are deduplicated across both boots. Reconstruction verifies every expanded byte against the original SHA-256 stored by the capture observer in `build/fd-cold-01/sha256.json`. The twelve images total 1,610,612,736 bytes. No captured RAM ranges were discarded.
- FD capture includes QMP commands, replies, inputs, raw UART, stderr, both scanouts, external fixture images, actual ELFs, contract and capture-time reader sources. The frozen reader walks the guest translation tables, reads protected reference ownership, compares physical XRGB8888 data to the external scanouts and checks the terminal process receipts. The QA runner separately summarizes the saved bytes and tests nine one-byte changes against the frozen reader after updating each changed file's digest. Its captured accepted stdout must reproduce byte for byte. An earlier incomplete QA transcript is retained under its own filename.
- The single window regression has eight raw PPMs, four PNGs, PL011 bytes `xnxq` and four process exits. The offline check recomputes each full RGB fixture with the captured observer, compares both reads and the decoded PNG, verifies input order and stable timing, and matches the UART against the original result.
- `build/` contains the original SHA inventories, manifests, ELF/readelf records, package logs, capture output, host identity records and generated Rust link-test input. The original kernel and role ELFs are retained in both package and capture form. `host/qemu-system-aarch64` contains the original executable bytes verified against both FD processes and the host before/after digests. The executable is archived as evidence with inert permissions. `source/` contains the committed build and observer paths; `git-trees/` authenticates their Git blobs back to the original commit object. `integration/` preserves the orchestration commands. Cargo's target caches, the compiler, host shared libraries and Git history outside these inputs are outside this evidence package.
- The evidence covers a single virtual board and QEMU TCG with a RAM framebuffer. Physical devices, GPU DMA, a hardware TLB, broader POSIX descriptor semantics and runtime execution of the separate Rust link fixture remain untested here. UART fixed-event buffers dropped a prefix in the FD boots; the surviving UART, terminal receipts, stopped raw RAM and QMP transactions are retained. Runtime counts are reported by the replay.

## 한국어 안내

AArch64 QEMU의 독립 부팅 두 건에서 소켓을 통한 파일 디스크립터 전달과 공유 화면의 보호·정리를 관측했다. 각 부팅에서 제한시간 직전, 대기열 등록, 수신 측 설치, 입력 전/후, 종료 시점의 물리 RAM 전체를 저장했다. 위 명령은 12개 원본을 복원하고 캡처 당시 SHA-256과 대조한다. 원래 관측기와 QA 프로그램을 실행해 출력 일치 여부 및 조작된 아홉 사본의 거부 원인을 확인한다. 창 데모 회귀 한 건은 외부 화면 전체, 입력 바이트, UART 종료 기록을 별도로 검사한다.

실제 하드웨어, GPU DMA 및 별도 Rust 링크 예제의 런타임 동작은 이 자료의 측정 범위 밖이다. FD 부팅의 고정 이벤트 로그에서 누락된 행 수는 재생 결과에 표시된다. 원본 ELF와 QEMU 실행 파일 바이트는 보존되어 있고 재생에는 GNU nm와 Python이 필요하다.
