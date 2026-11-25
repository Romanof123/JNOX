# Kernel profile shipping verification / 커널 빌드 프로파일 검증

Local a8037d2dc3758f02706eb3feef0a45424f3ca879, tree db16b7313ad2c263e71d1061fe173bba1bcab3e2. Default, explicit release and speed passed direct and ordinary native package builds. Each package artifact matched its profile path and exited 37 with the four expected UART lines in a cold single-core Cortex-A72 QEMU guest. Default and explicit release kernels were byte-identical. Speed differed and all three user ELFs matched. Three invalid selections returned status 2. No permit workload ran on this tree.

기본값·명시적 release·speed 각각 독립 빌드와 일반 네이티브 부팅을 통과했다. 세 경우 모두 예상 UART 네 줄과 종료값 37을 기록했다. 기본값과 release 커널 해시는 같고 speed 커널 해시는 달랐다. 앱 ELF 세 개는 동일했다. 지원하지 않는 선택 세 가지는 종료값 2로 거부됐다.

[Raw captures and offline replay](native-kernel-build-profile-selection.tar.gz) (SHA-256 `050e8806c7c50388552b901ab8079a1c2e86d24b55969b1d53ca4d62892bcdc4`).
