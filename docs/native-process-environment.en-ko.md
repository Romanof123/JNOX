# Package environment for native processes / 네이티브 프로세스의 패키지 환경

CHG-344 adds the per-process environment in IMP-344. EVD-344 records the bounded two-process startup and yield checks under VER-020. [Eight AArch64 guest cases](evidence/native-environment-462.en-ko.md)

The native C application requirement REQ-020 and startup design DES-005 govern the process entry interface. REQ-024 and DES-013 cover the local IPC endpoint used by the Wayland client. Check package and C entry behavior under VER-020, then the upstream display connection under VER-024. [Requirements](requirements.en.md)

Xwayland opens its display with `wl_display_connect(NULL)`. Its Wayland client library looks up `WAYLAND_DISPLAY` in the calling process. The native package can supply the display's absolute socket path to one process. The existing local-socket route and broker permissions still govern the connection. [Startup arguments](native-process-arguments.en-ko.md) · [Implementation and acceptance #462](https://github.com/Romanof123/JNOX/issues/462) · [Display integration #448](https://github.com/Romanof123/JNOX/issues/448)

```sh
python3 tools/build_native_el0.py --out env-pair --elf probe.elf --peer-elf probe.elf \
    --env 0:WAYLAND_DISPLAY=/wayland-0 --env 0:ROLE=display \
    --env 1:ROLE=control
```

This example packages the same ELF twice. Slot 0 receives two environment entries, and slot 1 receives only `ROLE=control`. The manifest records each slot's `environment` object. Values are part of the image and manifest and are visible to the package recipient. The packager takes only explicit entries. An absolute `WAYLAND_DISPLAY` lets the upstream client select the path without a runtime directory.

The slot number is a single digit from 0 to 4 and must name an explicitly supplied application. Each slot permits one value per ASCII name matching `[A-Za-z_][A-Za-z0-9_]*`. Values are UTF-8, can be empty and can contain further equals signs. Combined startup extras and environment entries have a limit of 15 strings and 1024 encoded bytes including trailing NULs. AMC generations, task-policy declarations and dynamically spawned shell children retain their existing startup contracts.

The native-v1 stack contains argc, argv pointers, a null pointer, envp pointers and a second null pointer. C applications can receive the third argument of `main`. The Picolibc graphics runtime installs its process-local `environ` before constructors can call `getenv`. Omitting the option preserves the existing empty-environment startup layout. The two-process, legacy and upstream client verification results are tracked in #462 and #448.

## 한국어

CHG-344의 프로세스별 환경 구현은 IMP-344에 해당한다. VER-020의 두 프로세스 시작 및 양보 시험 여덟 건은 EVD-344에 수록했다. [게스트 검증 기록](evidence/native-environment-462.en-ko.md)

패키지의 `--env 슬롯:이름=값`으로 각 프로세스에 필요한 환경을 지정한다. 위 명령은 같은 ELF를 두 번 배치한다. 슬롯 0만 `WAYLAND_DISPLAY=/wayland-0`를 받는다. 슬롯 1에는 `ROLE=control`만 전달된다. 각 슬롯의 설정은 이미지와 manifest에 포함되며 호스트 환경은 자동 전달되지 않는다. 소켓 연결에는 기존 브로커 설정과 접근 권한이 필요하다.

이름은 ASCII 식별자이고 같은 슬롯 안에서 고유하다. UTF-8 값에는 빈 문자열과 등호를 사용할 수 있다. 추가 argv와 환경을 합해 슬롯당 15개, 문자열 끝의 NUL을 포함해 1024바이트까지 허용한다. C 앱은 `main`의 세 번째 인수에서 읽을 수 있고 Picolibc 앱은 생성자에서부터 `getenv`를 쓸 수 있다. 옵션이 없으면 기존 빈 환경 레이아웃이 유지된다. AMC 세대와 태스크 정책, 셸에서 생성하는 자식은 기존 시작 규약을 사용한다. 대상 게스트 검증은 #462에서 추적한다.
