# 네이티브 POSIX 소스 API 프로파일 0.1

[English](posix-profile.en.md) · CHG-078 · [확장 사용자 API · #8](https://github.com/Romanof123/JNOX/issues/8)

**AArch64 확장 구성에서 제공할 계획이다.** 버전 0.1은 J-NOX SDK로 다시 빌드하는 C 프로그램에 제공할 소스 인터페이스를 정한다. 각 함수의 초기 상태는 *계획*이다. SDK·이미지·구성별로 타깃 사용자 모드 시험 결과가 확보된 함수만 검증 완료로 표시한다. 커널에 링크한 EL1 작업과 호스트 libc 시험은 각각 다른 인터페이스를 거친다. 인증과 libc 전체 범위에는 별도의 범위 정의 및 근거가 필요하다. [POSIX.1-2024, Issue 8](https://standards.ieee.org/ieee/1003.1/7700/) · [현재 플랫폼 근거](platform-checklist.md)

## 선택 함수와 타깃 검증

별도 EL0 프로그램을 SDK 헤더 `<fcntl.h>`, `<unistd.h>`, `<spawn.h>`, `<sys/wait.h>`, `<time.h>`, `<pthread.h>`, `<signal.h>`, `<poll.h>`, `<sys/socket.h>`와 선택한 주소 체계의 헤더로 컴파일한다. 타깃에서 해당 libc 진입점을 실행한다. 각 함수에 시험한 플래그·옵션, 반환값, 규정된 오류, 실제 상태 변화와 자원 회수 결과를 연결한다. 일반 시스템 호출 형태의 인터페이스는 규정된 실패 반환 뒤에 `errno`를 확인한다. 선택한 `pthread_*`, `posix_spawn*`, `sigwait`, `clock_nanosleep`은 오류 번호를 직접 반환한다. 성공한 호출에는 `errno` 검사가 필요 없다. 두 스레드에서 다른 실패를 유도해 각각 자기 `errno`를 읽는지 확인한다.

| 그룹과 추적 | 선택 인터페이스 | 타깃에서 관측할 결과 |
| --- | --- | --- |
| 파일·기술자 · VER-014/020 | `open`, `read`, `write`, `close`, `dup2`, `fcntl` | 허용 파일 생성·잘라내기·재열기, 정확한 바이트, 복제 기술자의 오프셋 공유, 부분 입출력, EOF를 확인한다. 없는 경로에서 `ENOENT`, 읽기 권한 거부에서 `EACCES`, 무효 기술자에서 `EBADF`를 확인한다. `F_GETFL/F_SETFL`로 `O_NONBLOCK`, `F_GETFD/F_SETFD`로 `FD_CLOEXEC`를 시험한다. |
| 파이프·자식 · VER-014/020 | `pipe`, `posix_spawn`, `posix_spawn_file_actions_init`, `posix_spawn_file_actions_destroy`, `posix_spawn_file_actions_adddup2`, `posix_spawn_file_actions_addclose`, `waitpid`, `_exit` | 파일 동작을 등록 순서대로 적용해 파이프 양 끝을 자식의 표준 입력·출력에 연결한다. 자식 둘과 부모가 쓰지 않는 끝을 닫는다. 버퍼의 데이터를 모두 읽은 뒤 마지막 쓰기 끝이 닫히면 읽기가 0을 반환해야 한다. 두 PID를 회수해 `WIFEXITED/WEXITSTATUS`로 상태를 읽고, 같은 PID를 다시 기다리면 `ECHILD`를 확인한다. 자식에서 close-on-exec 동작을 판정한다. |
| 시계 · VER-014 | `clock_getres`, `clock_gettime`, `nanosleep`, `clock_nanosleep` | 해상도, 감소하지 않는 단조 시간, 초기화된 달력 시각을 읽는다. 절대 단조 기한 대기 중 달력 시각을 변경해 원래 기한을 지키는지 본다. 핸들러가 있는 신호로 상대 대기를 중단하고 잔여 시간을 확인한다. 잘못된 나노초에서 `nanosleep`의 −1/`EINVAL`, `clock_nanosleep`의 직접 반환 `EINVAL`을 구별한다. [시간 계약](system-time-plan.md) |
| 스레드 · VER-005/014 | `pthread_create`, `pthread_join`, `pthread_mutex_init`, `pthread_mutex_destroy`, `pthread_mutex_lock`, `pthread_mutex_trylock`, `pthread_mutex_unlock`, `pthread_cond_init`, `pthread_cond_destroy`, `pthread_cond_wait`, `pthread_cond_timedwait`, `pthread_cond_signal`, `pthread_cond_broadcast` | 같은 프로세스의 동기화 객체와 합류 가능한 스레드를 쓴다. 스레드가 돌려준 데이터와 다른 스레드가 잡은 뮤텍스에서 trylock의 직접 반환 `EBUSY`를 확인한다. 조건을 반복 검사하면서 대기하고 한 대기자에게 signal, 여러 대기자에게 broadcast한다. 깨움 또는 직접 반환 `ETIMEDOUT` 후 뮤텍스 재획득을 확인하고 모든 사용이 끝난 객체를 폐기한다. 네이티브 협력형 두 스레드 구성에서는 NULL 속성으로 pthread_create를 호출하고 메인이 재사용 가능한 작업 하나를 pthread_join할 수 있다. [구성·제한·예제](native-joinable-threads.en-ko.md) · [EVD-125](evidence/evd-125.md) 명시적 프로세스 내 RR에서도 CPU0의 메인·작업자 조인을 확인했다. [EVD-126](evidence/evd-126.md) |
| 기본 신호 · VER-005/014 | `sigemptyset`, `sigaddset`, `sigaction`, `pthread_sigmask`, `sigwait`, `getpid`, `kill` | 스레드 시작 전에 `SIGUSR1`을 차단하고 프로세스로 보낸 뒤 보류 신호를 `sigwait`로 한 번 소비한다. 별도 사례에서 차단하지 않은 신호를 핸들러로 전달한다. `SIGPIPE`를 무시한 상태로 읽기 끝 없는 파이프에 쓰면 `EPIPE`로 실패해야 한다. 기본 처리 `SIGTERM`에 의한 종료를 `waitpid`와 `WIFSIGNALED/WTERMSIG`로 관측한다. |
| 준비 알림·소켓 · VER-006/014 | `poll`, `socket`, `bind`, `listen`, `accept`, `connect`, `send`, `recv`, `sendto`, `recvfrom`, `shutdown`, `close` | AF_UNIX 스트림과 IPv4·IPv6의 TCP 스트림 및 UDP 데이터그램에서 시험한다. `poll` 시간 초과·읽기 준비·닫힌 0 이상의 기술자(`POLLNVAL`), 스트림 부분 전송, 상대 종료 후 버퍼를 비웠을 때의 EOF, 비차단 would-block 결과를 본다. UDP 메시지 경계, 여러 클라이언트의 동시 실행, 부하 중 닫기, 포화 후 기술자·버퍼 회수를 검증한다. |

자원 시험에는 프로세스·스레드·기술자 수, 파이프 저장 공간, 소켓·큐·대기 요청 한도를 구성 명세로 고정한다. 하나씩 소진해 규정된 오류 또는 상한 있는 역압을 확인하고, 회수 후 같은 동작이 성공하는지 본다. 예를 들어 프로세스의 기술자 한도에 도달하면 `open`은 `EMFILE`로 실패한다. 설정별 한도와 회수 결과를 남긴다. 네이티브 IPC 소유권·취소·상대 종료는 REQ-024를 따른다. PM이 #21의 스케줄링 승인 근거를 연결한 뒤 #24에서 IPC 구현을 시작한다. [IPC 프로파일](ipc-mechanisms.md) · [개발 순서](product-plan.md)

## 애플리케이션 인수와 적용 범위

[최초 EL0 C 진입·종료 사례](native-el0-startup.md)

- **네이티브 C · VER-020.** 외부 정적 프로그램 `cat`, `wc`를 SDK로 각각 빌드한다. 타깃 셸에서 `61 0a 62 0a` 바이트에 `cat sample.txt | wc -l`을 실행해 줄 수 2, 빈 stderr, EOF, 자식 각각의 종료 상태 0을 확인한다. 파일이 없으면 생산자는 진단과 0이 아닌 상태를 내고 소비자는 EOF를 받아 종료해야 한다.
- **네이티브 Rust · VER-022.** 별도 Cargo 프로젝트에서 `core` 기반 정적 앱을 빌드한다. 공개 안전 SDK로 인자에 지정한 파일을 stdout에 복사해 상태 0으로 종료한다. 바이트 일치, 빈 stderr와 함께 비인가 파일의 거부, 0이 아닌 상태, 내용 미노출을 확인한다. `alloc`과 `std`는 별도 지원 상태를 공개한다.
- **기존 Linux ELF · VER-021.** 검증 계획에 고정한 Debian arm64 정적 BusyBox를 변경 없이 쓰며 Linux 게스트를 끈다. 정상 파일과 접근 거부 파일로 `wc -l sample.txt`의 출력·stderr·상태를 기준 Linux와 비교한다. 원시 Linux `-ENOSYS`와 잘못된 ISA 또는 인터프리터의 진입 전 거부도 시험한다. [정확한 표본·해시·판정 규칙](verification-plan.md)

프로세스 간 공유 동기화, robust 뮤텍스, 큐잉되는 실시간 신호, `fork`, 동적 링크, 셸·유틸리티 전체 범위는 이후 프로파일에서 채택 여부를 정한다. 생략한 옵션과 미지원 플래그를 SDK에 명시한다. 헤더, 공개 심볼, 링크된 실행 파일 단계에서는 타깃 검증을 대기로 기록한다. [요구사항](requirements.md) · [사용자 서비스와 ABI 설계](architecture.md)

## CHG-085 · 네이티브 정적 실행 파일 배치

최초 네이티브 AArch64 이미지 계약은 리틀엔디언 ELF64 ET_EXEC, 버전 1, 머신 183, System V 식별값, ABI 버전 0과 AArch64 플래그 0을 허용한다. 네이티브 기동 코드와 시스템 호출 규약은 SDK가 제공한다. ELF 식별값만으로 응용 ABI를 확인할 수 없다. 플래너는 불변 파일 바이트 및 호출자가 제공하는 페이지 정렬 사용자 가상주소 반열린 구간을 받는다. 기준 SDK 범위는 [0x00400000,0x00800000), 페이지는 4096바이트다. 링커 PHDRS를 명시해 예를 들어 0x00400000에 RX 코드를 놓고 쓰기 데이터는 별도 페이지에 배치한다.

가상주소 오름차순의 비어 있지 않은 PT_LOAD 1~8개, 프로그램 헤더는 전체 16개 이하를 허용한다. 권한은 R·RX·RW다. 가상주소/파일 시작 페이지 정렬, 한 페이지 이상인 2의 거듭제곱 정렬과 주소/오프셋 합동, 파일 크기 ≤ 메모리 크기, 파일 구간 유효성, 주소 연산 범위, 페이지 반올림 후 사용자 범위 포함과 페이지 간 비중첩을 확인한다. 진입 주소는 4바이트 정렬이고 최초 명령 전체가 파일에 들어 있는 RX 구간이어야 한다. 그 밖에는 PT_NULL 및 비어 있고 실행 권한이 없는 단일 선택 PT_GNU_STACK만 허용한다. 인터프리터·동적 링크·TLS 등 다른 헤더는 거부한다. 섹션 헤더·심볼·물리 주소는 사용하지 않는다. 파일 부분을 복사하고 메모리 꼬리와 노출되는 페이지 여백을 0으로 초기화한 뒤 최종 권한으로 매핑한다. 실패 시 일부 계획도 반환하지 않는다.

호스트 검사기와 향후 커널 매퍼는 같은 무할당 Rust 파서를 사용한다. SDK로 빌드한 C 이미지 계획을 링크 맵과 대조하고 형식·범위 대조 결과를 기록한다. 실제 매핑·스택·EL0 진입·시스템 호출·응용 결과는 별도 구현과 근거가 필요하다. [REQ-008/014](requirements.md) · [DES-004/005](architecture.md) · [VER-008/014](verification-plan.md)

실행 전에는 이미지 해시와 ABI 버전을 연결한 신뢰 패키지 매니페스트 등으로 실행 파일 및 네이티브 ABI 식별을 별도 인증해야 한다. 호스트 비교 이미지가 배치 검사를 통과해도 네이티브 실행 권한은 부여되지 않는다.

### SDK 이미지 확인

Rust가 설치된 환경에서 저장소 루트에서 검사기를 한 번 컴파일하고 SDK 링커가 만든 ELF 경로를 전달한다. 출력의 진입점, 권한, 복사·BSS 길이 및 페이지 끝을 SDK 링크 맵과 대조한다. SDK v1 기동 규약·적재 JNOX 노트·시스템 호출 ABI는 별도 패키지 검증 대상이다. 검사기는 노트를 검증하거나 응용을 실행하지 않는다.

```sh
rustc --edition=2024 tools/native_elf_plan.rs -o native_elf_plan
./native_elf_plan path/to/application.elf
rustc --edition=2024 --test kernel/native/lib.rs -o native_elf_tests
./native_elf_tests
```

[네이티브 AArch64 C 부트스트랩 ABI 및 텔레메트리 빌드](native-c-abi.md)

[텔레메트리 이식 시험과 호스트 검증 자료](telemetry-migration-pilot.md) · [SDK 정적 빌드 기록](evidence/native-sdk-static.en-ko.md)
