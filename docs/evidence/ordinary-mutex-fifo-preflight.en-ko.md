# FIFO integration build and host checks / FIFO 통합 빌드와 호스트 검사

The PM combined the two development patches on the accepted main and built the exact Git tree `a3875f86cbf3dffac46d9f7b2b2d7269331b2f45` of candidate [6f2f903623](https://github.com/Romanof123/JNOX/commit/6f2f903623031fee8d81a4413569a8dd74ca0bd4). The Rust 1.95.0 AArch64 release build used one Cargo job and the locked dependency set. The successful host harness ran 140 tests. The preflight ELF SHA-256 was `7060d8e7b964a1973a01d987c35d24ac6b84677d674b8515fcb5cfeb9350fc26`. Build directories may affect full ELF digests. The independent guest campaign records the hashes of the images it actually ran.

In original order, preflight.log records a shell invocation failure before compilation. compile.sh and preflight-compile.log record a successful release build followed by a host error when a test module was compiled directly. host.sh and host.log record the subsequent absence of the temporary directory. joint.sh and joint.log retain the clean build and successful `tools/scheduling_tests.rs` harness from a persistent ext4 directory. Reconstruct the source inputs with `git archive --format=tar a3875f86cbf3dffac46d9f7b2b2d7269331b2f45 kernel tools arch`. The machine-specific input and output paths in these recorded shell commands identify this run.

두 개발 변경을 결합한 후보를 Rust 1.95.0으로 빌드했다. 최초 셸 호출 오류, 개별 모듈의 독립 컴파일 오류, 임시 경로를 찾지 못한 시도와 정상 재실행을 원래 순서로 보존했다. 실제 스케줄링 테스트 진입점에서 140건이 통과했다. 이 묶음은 개발 통합 기록이며 QEMU 독립 판정과 그 실행 이미지 해시는 별도 최초 실행 보고서에 있다.

[Integration build and original host attempts / 통합 빌드 및 호스트 원본](ordinary-mutex-fifo-preflight.zip) · SHA-256 b6bed6e9c21503e6fe95b7031db38077d7d69e51357bb9065c487eac965acd7f.
