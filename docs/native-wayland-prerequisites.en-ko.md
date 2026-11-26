# Resident Wayland and Xwayland prerequisites / 상주 Wayland·Xwayland 실행 조건

On the frozen AArch64 SDK, Clang compiled a local stream client, a native page loan and a native waitset. The SDK lacks the descriptor transfer and shared file mapping interfaces used by the standard Unix `wl_shm` path. The selected display profile also admits only two processes. This inventory covers compilation and source inspection. The resident compositor, two isolated local clients and Xwayland still need a guest run. [#412](https://github.com/Romanof123/JNOX/issues/412) · [English roadmap](window-system-plan.en.md) · [한국어 계획](window-system-plan.md) · [Frozen source](https://github.com/Romanof123/JNOX/tree/fe2fb3f477f9bbfa48f572ef34430f190e2f151b)

고정된 SDK에서 로컬 스트림 클라이언트, 페이지 대여와 대기 집합 코드는 AArch64 오브젝트로 컴파일됐다. 표준 Unix `wl_shm` 경로에 필요한 파일 기술자 전달과 공유 파일 매핑 인터페이스가 없고, 현재 화면 패키지는 프로세스를 두 개까지만 수용한다. 아래 결과는 컴파일과 소스 검사 범위다. 상주 컴포지터·격리된 클라이언트 두 개·Xwayland의 동작은 게스트에서 검증해야 한다.

| Area / 항목 | Frozen native profile / 현재 구현 | What the selected port needs / 이식에 필요한 확인 |
|---|---|---|
| Unix socket | `AF_UNIX`, `SOCK_STREAM`, `socket` and `connect` compile. The native broker dispatches operations 1–9 and stores paths and bytes. 소켓은 클라이언트별 기술자 4개와 스트림 버퍼 64바이트를 제공한다. [SDK](../sdk/native-aarch64/local_socket.c#L13) · [Dispatch](../services/socket/src/main.rs#L79) · [Limits](../services/socket/src/local.rs#L3) | `sendmsg`, `recvmsg` and `SCM_RIGHTS` are undeclared. Receiving a Wayland `fd` requires ancillary descriptor delivery in wire order. 일반 바이트 스트림만으로는 수신자가 쓸 수 있는 기술자가 전달되지 않는다. [Wayland wire format](https://wayland.freedesktop.org/docs/book/Protocol.html#wire-format) |
| Shared buffers / 공유 버퍼 | `sys/mman.h` is missing for both `mmap` and `shm_open`. The native loan API transfers a 4 KiB page between one configured producer and consumer. 페이지 대여는 생성·게시·읽기·반환의 소유권 전환을 제공한다. [Loan SDK](../sdk/native-aarch64/include/jnox/loan.h) · [Route and pool](native-loan-ipc.en-ko.md) | `wl_shm.create_pool` receives an FD and a size so the compositor can map the backing object. A loan handle cannot be received and mapped as that FD. 파일 생성 방법은 선택할 수 있으며 `shm_open` 자체가 프로토콜 필수 호출은 아니다. [Wayland shared-memory contract](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_shm) |
| Event loop / 이벤트 루프 | `poll.h` and `sys/epoll.h` are missing. Native waitset sources are mailbox, pulse and event bits. 소켓의 비차단 모드는 제공하지만 대기 집합에는 소켓 기술자 등록 기능이 없다. [Native wait sources](../kernel/src/waitset_ipc.rs#L21) · [Socket mode](../sdk/native-aarch64/local-socket.md) | Verify readiness and wakeup for the chosen port’s descriptors. `epoll` is a Linux event backend choice. 포트에서 선택한 대기 방식으로 여러 클라이언트의 준비·깨우기를 검증한다. |
| Libraries and loader / 라이브러리·로더 | `wayland-client.h` and `wayland-server.h` are absent. The SDK links static ELFs. The guest requires AArch64 `ET_EXEC` (type 2) and rejects `PT_INTERP` and `PT_DYNAMIC`. [SDK build](../sdk/native-aarch64/build.sh#L32) · [Guest entry checks](../kernel/src/native_el0.rs#L884) · [ELF checks](../kernel/native/elf.rs#L72) | Pin and port the selected Wayland/Xwayland sources and dependencies for a supported link profile. 동적 라이브러리를 선택하면 로더 작업도 필요하다. 정적 링크를 선택해도 포팅한 라이브러리와 실행 검증은 필요하다. |
| Guest process count / 게스트 프로세스 수 | `--display-service` admits one or two single-threaded processes on virt FIFO. Ordinary `--socket-service` admits up to three processes. [Display profile](../tools/build_native_el0.py#L559) · [Socket profile](../tools/build_native_el0.py#L522) | The existing local broker, compositor and two isolated clients already need four slots before adding Xwayland. 먼저 이 구성을 한 이미지에서 수용하고 화면·포커스·버퍼 반환·재시작을 관찰해야 한다. [Stage criteria](window-system-plan.en.md) |

## Reproduce / 재현

From a repository containing the frozen tree, run the shell block below. Ubuntu Clang 14.0.0-1ubuntu1.1 generated three objects and nine compiler refusals. `readelf` identified the stream client as AArch64, type REL. The compiler used only the archived SDK headers. 고정 트리의 SDK 헤더만 사용한다. 오브젝트 세 개가 생성됐으며 나머지 아홉 항목은 각각 미선언 식별자 또는 헤더 누락 진단을 냈다.

~~~sh
set -eu
out=$(mktemp -d)
git archive d8c09d4189b9a72a5f8ed157f1e9fd1322eae446 sdk/native-aarch64/include | tar -x -C "$out"
probe() {
    printf '%s\n' "#include <$2>" "$3" > "$out/$1.c"
    if clang --target=aarch64-none-elf -std=c11 -ffreestanding -nostdinc -Werror \
        -I"$out/sdk/native-aarch64/include" -c "$out/$1.c" -o "$out/$1.o" 2>"$out/$1.err"
    then echo "$1 compiled"; else head -1 "$out/$1.err"; fi
}
probe unix sys/un.h 'int f(struct sockaddr_un *a) { int s = socket(AF_UNIX, SOCK_STREAM, 0); return connect(s, (struct sockaddr *)a, sizeof(*a)); }'
probe loan jnox/loan.h 'long f(void) { return jnox_loan_acquire().status; }'
probe waitset jnox/waitset.h 'long f(void) { return jnox_waitset_wait(0).status; }'
for sym in sendmsg recvmsg; do probe "$sym" sys/socket.h "void f(void) { (void)&$sym; }"; done
probe rights sys/socket.h 'int f(void) { return SCM_RIGHTS; }'
probe mmap sys/mman.h 'void f(void) { (void)&mmap; }'
probe shm_open sys/mman.h 'void f(void) { (void)&shm_open; }'
probe poll poll.h 'void f(void) { (void)&poll; }'
probe epoll sys/epoll.h 'void f(void) { (void)&epoll_wait; }'
probe client wayland-client.h 'void f(void) { (void)&wl_display_connect; }'
probe server wayland-server.h 'void f(void) { (void)&wl_display_create; }'
readelf -h "$out/unix.o" | awk '/Type:|Machine:/'
~~~
