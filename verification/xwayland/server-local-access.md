# Xwayland local host access on JNOX (#448)

The full-server build applies `server-local-access.patch` to the pinned Xwayland 24.1.6 source. Use the existing `server-lab.sh NEW_OUT REV` entry point and its documented runtime inputs. A JNOX target without local peer credentials requires the native cookie implementation from the combined #465 profile before it can start serving connections. [Authorization scope](https://github.com/Romanof123/JNOX/issues/465)

For the JNOX Unix-only transport, each listener calls upstream's DefineSelf entry point to record one zero-length FamilyLocalHost in selfhosts. MakeHost initializes its requested flag to false. Upstream AddLocalHosts, DisableLocalHost and InvalidHost retain responsibility for enabling or removing access. Hostname-based IPv4/IPv6 matching has no eligible peer on this transport. TCP-enabled JNOX and other systems use the original interface and DNS logic.

## Build evidence

The combined Xwayland, native authority and selected-keymap build
compiled access.c, auth.c, mitauth.c, connection.c and utils.c. The
AArch64 access object has no unresolved resolver or IP interface query.
The software target omits the optional shared-fence backend when
libxshmfence is absent, matching the existing upstream screen-init
guard. The target reports the unsupported `-core` command-line option.
The first remaining compiler error on the measured integration revision
`c45b49d` was an unconditional `xf86drm.h` include in the software
window header. Full server linking and display startup are still open.
[Server issue](https://github.com/Romanof123/JNOX/issues/448)

The subsequent integrated build compiled that window header, the input
callback and the software Present code. [Later compiler results](server-executable.md)

## Remaining guest checks

- Call DefineSelf twice and inspect one FamilyLocalHost entry. With access enabled and an empty explicit host list, local access enabled then disabled must respectively admit then reject the AF_UNIX peer.
- With forced authority enabled, verify valid-cookie X11 setup and a successful request. Missing and incorrect cookies must each fail setup on the same local listener.

## 한국어

JNOX의 AF_UNIX 전용 빌드에서는 원본 서버가 IP 인터페이스를 열거할 필요가
없다. 각 리스너는 `DefineSelf`로 로컬 호스트를 등록하고, 이미 등록된
항목은 재사용한다. 등록 시점의 접근 허용 여부와 인증 쿠키 검사는 기존
서버 코드가 담당한다. 서버 전체 통합 시험에서는 로컬 호스트의 중복 등록,
접근 허용 및 철회, 올바른 쿠키의 요청 성공, 잘못되거나 누락된 쿠키의
연결 거부를 각각 확인한다.

실제 서버 빌드는 접근·연결·인증·유틸리티 소스를 AArch64 오브젝트로
컴파일했다. 통합 리비전 `c45b49d`에서 남은 첫 오류는 소프트웨어 창
헤더의 `xf86drm.h` 의존성이었다. 아래 단계인 서버 링크, 부팅, 키 입력과
Mosaic 화면 검증은 계속 추적한다.
