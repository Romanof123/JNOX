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
