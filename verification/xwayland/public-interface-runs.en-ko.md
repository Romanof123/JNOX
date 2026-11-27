# Native interface guest results / 네이티브 인터페이스 게스트 검증

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. These serialized AArch64 QEMU virt runs exercised C applications and native JNOX services. Exit codes listed together are in ascending kernel slot order, starting at slot 0. Each row identifies the exact local Git revision used to build the public-source candidate. [OS interfaces #471](https://github.com/Romanof123/JNOX/issues/471) · [Filesystem #459](https://github.com/Romanof123/JNOX/issues/459)

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

The repeat used kernel ELF SHA-256 `c40349e5e0e55a2a6b0ce98f7f3e42db68ef8b6824090e4167e7559d64f66daa`. The guest observed volatile contents being discarded when the virtual machine restarted. Persistent media and power-loss behavior have separate acceptance criteria. [Fixture and commands](tmpfs/README.en-ko.md) · [Control signal profile](signals/delivery.en-ko.md) · [Process groups](process-id/groups.en-ko.md) · [UTC source](wall-clock/README.en-ko.md)

게스트 종료값은 커널 슬롯 0번부터 순서대로 적었다. `3e3a740`에서는 신호 기본 동작으로 종료한 프로세스가 SIGTERM의 예상값 143을 남겼고, 별도 부팅에서 부모가 잠든 자식에 신호를 보내는 경로와 프로세스 그룹 수명을 확인했다. UTC를 지정한 경우와 지정하지 않은 경우도 각각 실행했다.

`da86130`의 네 프로세스 시험에서 쓰기 권한 앱이 만든 파일을 읽기 권한 앱이 열었고, 삭제 요청은 거부됐다. 같은 커널을 두 번 콜드 부트했을 때 두 번째 앱은 첫 번째 부팅에서 남긴 경로가 사라진 것을 확인했다. 이 표의 수용 범위는 명시한 소스와 가상 장치의 인터페이스 동작이다. 서버 전체 기동과 실제 화면·입력에는 별도 게스트 관측이 필요하다. [창 시스템 #448](https://github.com/Romanof123/JNOX/issues/448)
