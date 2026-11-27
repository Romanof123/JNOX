# Xwayland dependency compiler milestone / Xwayland 의존성 컴파일 기록

The serial production build targeted Xwayland 24.1.6 with Clang 14.0.0 for aarch64-none-elf and JNOX Picolibc 1.8.10. It used the staged Wayland 1.23.1 target and host scanner. It produced libxcvt 0.1.3, libmd 1.1.0 and libXau 1.0.11 target archives, linked the CVT probe, and generated XCB protocol sources from xcb-proto 1.17.0. libxcb 1.17.0 compiled xcb_conn.o and xcb_out.o. The attempt stopped in xcb_in.c at five calls to pthread_cond_signal. The Xwayland server configuration, link and guest launch were still pending at this milestone.

네이티브 JNOX용 정적 libxcvt, libmd, libXau와 CVT 검사 실행 파일을 만들었다. 실제 XCB 프로토콜 코드를 생성하고 원본 xcb_conn.c의 전송 경로를 네이티브 sendmsg에 연결했다. xcb_conn.o와 xcb_out.o가 완성되었으며, xcb_in.c에서 pthread_cond_signal 선언과 구현이 필요해 작업이 중단됐다. 서버의 Meson 구성, 링크, 게스트 실행은 이 단계 이후의 검증 대상이다.

| Attempt / 시도 | Observed outcome / 확인 결과 |
| --- | --- |
| 01 | The driver looked in the wrong Python package directory after installing host xcbgen. 호스트 모듈의 실제 설치 경로를 찾아 수정했다. |
| 02 | Libtool rejected startup objects while building libmd.la. 시작 객체를 최종 실행 파일 링크에서만 사용하도록 수정했다. |
| 03 | The first libxcb target compiler errors in xcb_conn.c were the missing netinet/in.h header and undeclared writev. SDK 주소 헤더를 내보내고 실제 소켓의 벡터 쓰기를 sendmsg로 연결했다. |
| 04 | xcb_in.c lines 111, 162, 326, 344 and 938 called the missing pthread_cond_signal. 해당 호출 다섯 곳의 진단을 compiler.txt에 남겼다. |

compiler.txt contains bounded excerpts from the four original logs, with the lab root shortened. SHA256SUMS identifies the original logs, source revision records, target archives, ELF and objects. The full files remain in the local evidence collection.

## Replay / 재현

From a checkout of the published source, set the input roots and staged prefixes described in server-executable.md. The output directory must be fresh.

    bash verification/xwayland/server-lab.sh "$NEW_EXT4_OUTPUT" HEAD

compiler.txt identifies the local historical experiment revisions. The archive and object digests apply to the preserved build paths and toolchain. The CVT probe has link evidence at this milestone.

공개 소스 체크아웃에서 두 경로를 설정하고 위 명령을 실행한다. 과거 실험의 커밋은 로컬 기록이다.
