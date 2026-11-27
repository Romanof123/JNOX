# Native interface guest results / 네이티브 인터페이스 게스트 검증

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. These serialized AArch64 QEMU virt runs exercised C and Rust applications and native JNOX services. Exit codes listed together are in ascending kernel slot order, starting at slot 0. Each row identifies the local Git revision used to build the public-source candidate. [OS interfaces #471](https://github.com/Romanof123/JNOX/issues/471) · [Filesystem #459](https://github.com/Romanof123/JNOX/issues/459)

The published commits below have identical Git trees to their tested local revisions. [`fd6657c5` source](https://github.com/Romanof123/JNOX/tree/6f8615a4a3856c1e8f7cdaa6ff7644f45158a136) · [`3e3a740` source](https://github.com/Romanof123/JNOX/tree/2407815ca9599d1cead965c948232a0ecada71b2) · [`dcf8a47` source](https://github.com/Romanof123/JNOX/tree/961a415b4c653338f92bd348a59eef578fd9ce0e) · [`da86130` source](https://github.com/Romanof123/JNOX/tree/fb5bb78467140a476605b27bdc881671fbb3eb84)

| Source revision | Guest selection | Observed exit codes |
| --- | --- | --- |
| `fd6657c5` | One Picolibc caller exercised six cases in one boot: clock resolution, shrink seals, descriptor limit, two-second sleep, anonymous file offsets and service credentials. This revision has the same tree as public PR #473. | Caller 86. [Six-case receipt](https://github.com/Romanof123/JNOX/pull/473) |
| `3e3a740` | Combined Picolibc interface checks; selected signal dispositions/handler return; default SIGTERM. | Interfaces 86, 0. Signals 86, 0. Default SIGTERM 143, 0. |
| `3e3a740` | Parent sent a signal to a sleeping child. A separate three-process run exercised the process-group lifecycle. | Signal delivery 83, 84, 85. Groups 90, 91, 92. |
| `3e3a740` | UTC provisioned at boot; UTC unset in a separate package. | 87, 0 in each boot. |
| `dcf8a47` | Volatile file service with a read/write grant, a read-only grant and an unset UTC source. | 88, 0 in each of three boots. |
| `da86130` | Directory and file operations with read/write and read-only grants. | 89, 0 in each of two boots. |
| `da86130` | Concurrent writer, reader, client without a grant and broker. The reader observed the writer's bytes and was denied unlink. | 88, 88, 88, 0. |
| `da86130` | Two successive cold boots of the same kernel image. Each caller required an initially absent file and left that pathname present before exit. | 88, 0 in both boots. |
| `ce9b35f` | Rust writer, read-only Rust reader, routed Rust client without a mount grant, and broker. Reader checked the writer's six bytes and denied mutations. | 90, 90, 90, 0. |
| `ce9b35f` | Corrected C path fixture with read/write and read-only grants. Metadata calls checked live sockets/backings and closed or unallocated descriptor numbers. | 89, 0 in both boots. |

The repeat used kernel ELF SHA-256 `c40349e5e0e55a2a6b0ce98f7f3e42db68ef8b6824090e4167e7559d64f66daa`. The guest observed volatile contents being discarded when the virtual machine restarted. Persistent media and power-loss behavior have separate acceptance criteria. [Fixture and commands](tmpfs/README.en-ko.md) · [Control signal profile](signals/delivery.en-ko.md) · [Process groups](process-id/groups.en-ko.md) · [UTC source](wall-clock/README.en-ko.md)

The three `ce9b35f` guests used the [published source tree](https://github.com/Romanof123/JNOX/tree/e3372a00a7b7797880f1af5b2be0211157aa8abe). Their kernel ELF SHA-256 values follow. Both C builds produced the same graphics adapter, `8563d0e22e7b471c4ffb781c2d9e357edd0fe992e3406b6820403ff83f967c50`, now pinned in the server builder. [Rust API and test command](tmpfs/rust-sdk.en-ko.md) · [C fixtures](tmpfs/README.en-ko.md)

| Guest | Kernel ELF SHA-256 |
| --- | --- |
| Rust file API, four roles | `c1e53d0038b16d03a6cf247442410d87468eafff92863505ad2259810d05878a` |
| C path operations, read/write | `6ce4491c2abe8783bfa7943c8f1cccf9ffe16531c0ddf9aa82534c805ca6a20c` |
| C path operations, read-only | `12207aaf1fdb021f5d180bb85841bb09481d95a6ef35a0aa41a6b59f66f8c10d` |

게스트 종료값은 커널 슬롯 0번부터 순서대로 적었다. `3e3a740`에서는 신호 기본 동작으로 종료한 프로세스가 SIGTERM의 예상값 143을 남겼고, 별도 부팅에서 부모가 잠든 자식에 신호를 보내는 경로와 프로세스 그룹 수명을 확인했다. UTC를 지정한 경우와 지정하지 않은 경우도 각각 실행했다.

`da86130`의 네 프로세스 시험에서 쓰기 권한 앱이 만든 파일을 읽기 권한 앱이 열었고, 삭제 요청은 거부됐다. 같은 커널을 두 번 콜드 부트했을 때 두 번째 앱은 첫 번째 부팅에서 남긴 경로가 사라진 것을 확인했다. 이 표의 수용 범위는 명시한 소스와 가상 장치의 인터페이스 동작이다. 서버 전체 기동과 실제 화면·입력에는 별도 게스트 관측이 필요하다. [창 시스템 #448](https://github.com/Romanof123/JNOX/issues/448)

공개 소스 `ce9b35f`의 게스트에서 Rust 쓰기·읽기·권한 없음 앱이 각각 90, 브로커가 0으로 종료했다. 파일 디스크립터 오류 분류를 수정한 C 경로 시험도 두 권한 구성에서 모두 89, 0을 기록했다. 두 C 빌드에서 실측한 동일 그래픽 어댑터 해시를 서버 빌드에 고정했다.

## Integrated server ELF / 통합 구성 서버 ELF

Integration revision `f2e4f3d` linked the original Xwayland 24.1.6 software target into a 10,826,992-byte static ELF64 AArch64 executable, SHA-256 `033344648cb13a4bc011c3c057054c0164426afd6ee171cf9ba4a32418dad1f3`. All required imports resolved. ELF inspection found no runtime relocations, interpreter, dynamic segment or TLS segment. The only remaining undefined symbol is Picolibc 1.8.10's optional weak `_user_strerror`: `strerror.c` checks the weak address before using the application extension for an unknown error. The normal builder now calls the [ELF verifier](server_receipt.py). The integrated source includes the five-role launch work tracked in #448. This receipt covers its static link; guest startup, actual pixels, input and restart require their own observations. [Guest server #448](https://github.com/Romanof123/JNOX/issues/448)

Two later cold boots of the five-role integration package at `a41c94a` ran Xwayland into Xtrans listener initialization. The non-root server encountered a missing `/tmp/.X11-unix` before authenticating any client. Mosaic rendering, input and restart remain open. [Original startup observations](https://github.com/Romanof123/JNOX/issues/448#issuecomment-7217490129)

통합 구성 `f2e4f3d`에서 원본 Xwayland의 필수 심볼이 전부 연결됐다. Picolibc의 선택 확장 함수 `_user_strerror`는 사용 전 주소를 확인하며, ELF에는 약한 미정의 심볼로 남는다. 후속 다섯 역할 통합 게스트 두 부팅에서 Xtrans 리스너 초기화까지 실행했다. 일반 권한 서버가 아직 없는 소켓 디렉터리를 생성하는 단계에서 멈춰 인증과 창 출력·입력·재시작은 남아 있다.
