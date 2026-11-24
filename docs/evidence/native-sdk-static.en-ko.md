# Static native SDK images / 네이티브 SDK 정적 이미지

The SDK at `18bfadf2de4f2662a69ddfed471312850ca0a32b` produced two static AArch64 ELFs with its published build.sh and Ubuntu clang/lld 14.0.0. The telemetry source is the unmodified #171 blob 2b6a71bcfb7274a853c723ef26a74d2d5f381dea. The second input was native_startup_probe.c blob 622b4c7dd9783e4a9d0c443b8763dfa034e6bb63. Source files were taken from Git archives. The #173 inspector at `d63cf59c096e236a0f3f7cba588975fc658dd8b7` built on the Windows host with Rust 1.95.0 and accepted both. Its three original regressions passed (copy and clear lengths, file-backed entry, permission/window checks).

| Image | RX copy | R copy | RW copy + zero | SHA-256 |
|---|---:|---:|---:|---|
| telemetry_monitor | 1956 | 133 | 0 + 4 | d836aa80b4452a0171836362d73b645704a546a356f82f88d324f31f1583876d |
| native_startup_probe | 1000 | 36 | 4 + 12 | f80ce35a07cb8a3275313ce74d114dbb782d19eed4624aa0b115134e0d88b195 |

GNU readelf agrees with the inspector on start 0x400000 and three distinct 4 KiB pages at 0x400000/0x401000/0x402000. The only program headers are RX, R and RW loads. The read-only JNOX note has type 1 and little-endian descriptor 1. Objdump shows _start clearing x29/x30, loading argc from [sp], passing argv as sp+8, calling main, sign-extending its return and issuing exit via x8=1 and svc #0x4a01. Stack contents, SP alignment at handoff, applied page permissions, live ABI calls and process status require a guest measurement.

SDK의 공개 빌드 스크립트로 수정하지 않은 텔레메트리와 시작 시험 프로그램을 정적 링크했다. Rust 검사기와 GNU readelf의 진입점·적재 영역·복사 및 초기화 길이가 일치했다. 세 영역은 실행/읽기, 읽기 전용, 읽기/쓰기이며 동적 인터프리터가 없다. 읽기 전용 영역에 ABI 버전 1 노트가 있다. 시작 코드에서 스택의 argc와 argv를 main에 전달하고 반환값을 종료 시스템 호출에 전달하는 경로를 확인했다. 실제 스택 배치·페이지 권한·종료 상태는 게스트 실행에서 확인해야 한다.

The initial pre-public working-copy script had a trailing CR on its output argument. The measured canonical Git archive emitted the expected filenames. ZIP SHA-256 `dc34696c8d7809d3dff0fac15434f4c4d993a9b27019033be9c276aaa1bda1c9`. [ELFs, inspector output, GNU headers, disassembly and tests / ELF·검사 결과·역어셈블리·회귀 시험](native-sdk-static.zip)
