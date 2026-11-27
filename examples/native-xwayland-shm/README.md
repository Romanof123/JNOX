# Native anonymous pixmap backing

This fixture exercises the Xwayland 24.1.6 anonymous-file sequence on JNOX.
Use the build and capture procedure in [the descriptor-transfer fixture](../native-fd-transfer/README.md), substituting this directory's `main.c` and producer argument `m`. Keep the consumer, unrelated client, broker, routes, shared-FD option and interactive frame gates unchanged. Inspect `jnox_xwl_shm_probe` in each process's address space. Producer words 0, 3 and 7 must be 1. Consumer words 4 and 5 must be 1. Failure words 1 and 2 record the producer source line and errno. Verify the ordinary transfer witnesses, framebuffer contents and final process exits as well.

With the local-socket SDK, the selected upstream path uses `memfd_create("xwayland-shared", MFD_CLOEXEC | MFD_ALLOW_SEALING)`, `fcntl(F_ADD_SEALS, F_SEAL_SHRINK)`, `ftruncate`, whole-file `MAP_SHARED` and exact-length `munmap`. Select the upstream ftruncate branch when configuring. Growth zero-fills newly exposed bytes. A mapping keeps its backing alive after close. Transferred and duplicated descriptors share size and seals, retain their access permissions, and have a descriptor-local close-on-exec flag. The current static native image has no exec transition.

The profile supports objects up to 307200 bytes, a zero-length initial object, and SEAL, SHRINK and GROW seals. It rejects size changes to a mapped object with EBUSY and rejects unsupported seals, memfd flag combinations, partial mappings, offsets, address hints and partial unmaps. The original fixed-size jnox_shm_create objects continue to reject zero length and do not support resizing or seal operations. This fixture exercises the anonymous pixmap file operations and passes the resulting backing through the ordinary SCM_RIGHTS and framebuffer pipeline. Building and launching the upstream server requires additional port work.

Run once with the default consumer arguments and again with consumer argument c. Retain producer argument m. Consumer word 6 must be 0 and then 1. Flags 0 and MSG_DONTWAIT install an FD with FD_CLOEXEC clear. MSG_CMSG_CLOEXEC requests atomic installation with FD_CLOEXEC set. Sender flags and access permissions are independent. Both runs validate rejection of unsupported flags before queue consumption, send-side rejection of MSG_CMSG_CLOEXEC, empty receive COMMIT timeout, no unintended FD installation, exact stream bytes and the read-only grant.

## 지원 범위와 검증 방법

네이티브 게스트에서 Xwayland 24.1.6의 익명 픽스맵 파일 생성 절차를 실행한다. 위의 빌드 절차에서 생산자 인자를 m으로 지정한다. 초기 크기가 0인 memfd에 축소 방지 봉인을 먼저 적용하고 ftruncate로 공간을 확보한다. 지원하는 최대 크기는 307200바이트다. 확장으로 새로 노출되는 바이트는 0으로 채운다. 쓰기 가능한 파일 기술자만 크기와 봉인을 변경할 수 있다. 객체에 매핑이 있으면 크기 변경은 EBUSY로 실패한다. 읽기 전용 별칭, 전송된 기술자, 열린 매핑은 같은 객체를 참조한다. 마지막 파일 기술자를 닫은 후에도 기존 매핑은 유지된다.

MAP_SHARED로 파일 전체를 오프셋 0에서 매핑하고 정확히 그 길이로 해제할 수 있다. 부분 매핑·부분 해제, 주소 힌트, 지원하지 않는 memfd 플래그 조합과 봉인은 오류를 반환한다. 고정 크기 방식인 기존 jnox_shm_create의 크기 제한과 권한은 그대로 적용된다. 현재 정적 네이티브 이미지에는 exec 전환이 없다.

SCM_RIGHTS 수신에 MSG_CMSG_CLOEXEC를 지정하면 기술자를 설치하는 동시에 FD_CLOEXEC를 설정한다. 기본 수신 또는 MSG_DONTWAIT만 지정하면 비트는 0이다. 두 경우 모두 읽기 전용 권한은 유지된다. 지원하지 않는 수신 플래그는 데이터를 소비하기 전에 거부한다. 송신 시 MSG_CMSG_CLOEXEC를 사용하면 ENOTSUP를 반환한다. 소비자 인자를 생략한 경우와 c로 지정한 경우를 각각 실행하며 생산자 인자는 m으로 유지한다. 생산자 관측값 0·3·7 및 소비자 관측값 4·5는 모두 1이어야 한다. 소비자 관측값 6은 첫 실행에서 0, 두 번째 실행에서 1이어야 한다. 빈 큐 수신 시간 만료, 이후 전송 바이트와 기술자 권한, 기존 화면과 종료 코드도 확인한다. Xwayland 서버 부팅은 별도로 검증해야 한다.

## Duplicate a shared FD before marshalling

Producer argument `d` selects the duplication gate. Build this main.c with sdk/native-aarch64/build.sh --local-socket, then follow the four-slot packaging, FIFO priorities, IPC routes, display setup and two interactive frame checkpoints in ../native-fd-transfer/README.md. Substitute `d` for producer slot 1's `p`; use the existing consumer and unrelated roles. Retain the captured UART, stopped RAM, both framebuffer images and exact kernel/role ELFs. Run the existing transfer pixel/authority checks and:

```sh
python3 examples/native-xwayland-shm/check_dup.py /path/to/uart.bin --mutations
```

Wayland 1.23.1 src/connection.c duplicates each outgoing h argument through wl_os_dupfd_cloexec(fd, 0). Its src/wayland-os.c first calls fcntl(F_DUPFD_CLOEXEC) and falls back on EINVAL to F_DUPFD plus F_SETFD. The native SDK defines F_DUPFD as 0 and F_DUPFD_CLOEXEC as 1030. Picolibc 1.8.10 exposes 0 and 14 with POSIX.1-2008 interfaces enabled. The graphics wrapper translates between these two ABIs before sending existing shared-file duplication SVC 188. x0 carries the source FD, x1 carries a tagged ordinary/CLOEXEC selector, and x2 carries the signed minimum. For historical read-only duplication x1=0 and x2 is ignored. Other unreserved x1 values also retain this historical attenuation. The two reserved invalid selector values return EINVAL. SVC 197 remains exclusively assigned to the tablet profile. A wrong or unallocated source in the shared table returns EBADF. A live socket returns ENOTSUP, a closed socket EBADF. Other providers retain their existing fcntl behavior.

The shared provider allocates descriptors 96 through 103 only. Under the existing masked kernel operation it checks the source authority, rejects invalid flags, rejects negative minima or values above 103 with EINVAL, and installs the same object and writable permission in the lowest free slot at or above the minimum. A minimum below 96 searches from 96. A valid lower bound with no available slot returns EMFILE. Detection happens before table mutation. Descriptor-local close-on-exec is set atomically, independent of the source flag. Object size and seals are shared across aliases. This mmap-only provider has no seekable read/write cursor to duplicate; positional reads/writes and nonzero mmap offsets retain their previous behavior. The new slot is an ordinary reference in the existing descriptor table; existing mappings, queued rights and process teardown retain their original lifetimes.

The guest checks minima -1, 0, 95, 96, 98, 100, 101, 103 and 104, invalid raw flags, full tables with and without a lower hole, read-only mapping denial, seals, mapping lifetime after closing all slots, socket rejection, and final role exits. It duplicates the real frame's read-only descriptor, closes the original and sends the duplicate to the ordinary SCM_RIGHTS consumer. The UART checker mutates each of 16 recorded observations independently. This candidate has source preparation only; no new boot, capture or mutation result has been measured.

## 한국어

Wayland는 송신할 FD를 보관할 때 최솟값 0으로 F_DUPFD_CLOEXEC를 호출한다. JNOX는 공유 파일 전용 슬롯 96~103에서 하한 이상인 가장 작은 빈 번호를 고른다. 96보다 작은 유효한 하한은 96부터 검색한다. 하한이 음수이거나 103을 넘으면 EINVAL이며, 유효한 범위에 빈 슬롯이 없으면 EMFILE이다. 객체·권한·봉인은 유지되고 CLOEXEC는 설치 시 원자적으로 지정되며 원본과 독립적이다. 공유 파일은 메모리 매핑 전용이므로 읽기·쓰기 파일 위치는 없다. 새 생산자 인자 d는 원본을 닫은 뒤 복제 FD로 실제 읽기 전용 프레임을 전달하고, UART·화면·종료 상태로 검사한다. 이 변경의 실행 검증은 대기 중이다.

Before combining profiles, run `python3 examples/native-xwayland-shm/check_dispatch.py --tablet-ref ff1f2f940bab59d4dd5e540f51cd296cae1aea8a`. The guard checks the complete shared descriptor call-number set and verifies that the tablet revision owns 197. The allocation of top-level native SVCs is unchanged. The four extended selector values inside shared-FD SVC 188 are 0x4a464400 (plain), 0x4a464401 (close-on-exec), and 0x4a464402/403 (invalid). No descriptor has a shared seek offset in this mmap-only profile; the only supported file mapping offset remains zero.

기존 공유 파일 복제 SVC 188의 하위 선택자로 구현했다. 태블릿 SVC 197과 겹치지 않는다. x1=0인 기존 호출은 읽기 전용으로 권한을 줄이며, 새 호출도 원본보다 높은 권한을 부여하지 않는다. 공유 가능한 파일 위치 상태는 없고 매핑 오프셋은 0만 지원한다.

### Serialized upstream duplication gate

The earlier provisional SVC 197 candidate was caught before publication. The integrated source uses shared-FD SVC 188 and preserves tablet SVC 197. The gate starts from a clean integrated Linux checkout and records the exact commit, tree, archived Wayland source hash, linked symbol, QEMU identity, original UART and both physical capture sets. It serializes the whole campaign using the existing gate lock.

```sh
bash examples/native-xwayland-shm/run_dup_gate.sh \
  /mnt/d/codex/jnox448-xwayland/dep-inputs/wayland/wayland_1.23.1.orig.tar.gz \
  /mnt/d/codex/jnox444-independent \
  /home/ywcho/jnox406-lab/448-graphics-93e41a9/attempts/attempt05/runtime \
  /home/ywcho/jnox406-lab/448-dup-fresh-output /home/ywcho/jnox406-lab/run.lock
```

The build extracts the upstream function pointer, unmodified set_cloexec_or_close and wl_os_dupfd_cloexec bodies from src/wayland-os.c, and the exact minimum-zero expression from src/connection.c. It links these into both the native C SDK frame producer and a separate Picolibc caller. This exercises the exact descriptor-duplication path; running the complete libwayland closure/dispatch and Xwayland server remains a separate gate. A short-lived reservation makes the original read-only FD 98 and the outgoing duplicate 97, allowing the independent historical physical-memory/PCAP-free framebuffer oracle to keep its existing FD reuse expectations. The package runs two cold boots and the independent before/after/cancellation/authority checks and per-field mutations before the Picolibc process. Its raw terminal exit must be 74. Neither a compilation nor guest run was performed while preparing this gate.

직렬 게이트는 배포 전 발견된 197번 충돌을 피한 188번 구현으로 실행한다. 원본 Wayland 복제 함수와 하한 0 호출식을 추출하여 SDK와 Picolibc 호출자에 각각 링크한다. 기존 검사기의 슬롯 배치를 유지하고, 두 번의 콜드 부트와 픽셀·권한·변조 판정 및 별도 Picolibc 프로세스의 종료 코드 74를 요구한다.
