# Seekable anonymous files / 익명 파일 오프셋과 입출력

CAP-007 · REQ-010/011/014 · CHG-340 · REQ-015/024 · VER-015/024. [Filesystem #459](https://github.com/Romanof123/JNOX/issues/459) · [Native Xwayland imports #471](https://github.com/Romanof123/JNOX/issues/471)

The shared-FD AArch64 profile exposes `lseek`, `read` and `write` on a resizable anonymous file created by `memfd_create`. The caller needs its existing shared-FD route and a live descriptor. A created object has a single open file description. Ordinary duplicates, read-only aliases and descriptors passed through SCM_RIGHTS refer to its file position. A separately created file starts at position zero. Closing the last reference permits reuse with a new position, size and seal state. Files opened by pathname, including fonts and server lock files, require the namespace stage of #459.

| Operation | Native contract |
| --- | --- |
| Seek | SET, CUR and END accept a signed LP64 offset. A position beyond EOF leaves the size unchanged. Negative results return EINVAL; arithmetic overflow returns EOVERFLOW. |
| Read | Transfers up to 256 bytes per call. At or beyond EOF the result is zero. |
| Write | Transfers up to 256 bytes and advances the file position by the actual count. Extension zeroes holes and newly exposed tail bytes, up to the 307,200-byte backing limit. At that limit a further write returns EFBIG. |
| Rights and lifetime | A read-only descriptor rejects writes with EBADF. Growth after F_SEAL_GROW returns EPERM. Whole-file mappings block growth with EBUSY. In-place writes remain visible to existing mappings. A rejected copy preserves the position and file length. |
| Other providers | A live AF_UNIX socket returns ESPIPE for seek. Fixed-size shared backing retains its mapping-only byte access. |

SVC 202 uses x0 for the shared descriptor and x2 for the signed offset or user buffer address. Seek uses selector 0, 1 or 2 in x1. Read and write use `(count << 8) | 3` and `(count << 8) | 4`. The kernel validates the fixed transfer limit and copies each transfer through a stack buffer before committing position or size. The stack copy permits the user buffer to alias a mapping of the same backing. The existing `ftruncate` extent and seal rules also govern implicit write growth.

For the native guest gate compile [memfd_io_caller.c](../../sdk/native-aarch64/graphics/os/memfd_io_caller.c) against the installed Picolibc graphics headers. Package it in the FIFO slot with a directed route to the Rust socket broker and `--shared-fd` as in the [two-role Picolibc gate](../../examples/native-xwayland-shm/README.md). The expected PID 0 exit is 76. Failures 101–119 identify an assertion group in the source. The caller exercises ordinary and read-only aliases, independent creation, a sparse hole, truncation, overlapping mapped buffers, seal rejection, bad user buffers, short I/O, stale descriptors, fixed buffers and reuse. Cross-process offset sharing requires an additional SCM_RIGHTS guest run.

네이티브 AArch64 공유 FD 프로필에서 `memfd_create`로 만든 익명 파일의 탐색·읽기·쓰기를 제공한다. 같은 파일의 복제 FD는 오프셋을 공유하고, 새로 만든 파일은 0부터 시작한다. 파일 끝을 넘긴 탐색은 크기를 바꾸지 않는다. 이후 쓰기는 중간을 0으로 채우며 축소·확장 봉인과 기존 매핑의 수명을 따른다. 한 호출의 사용자 데이터 복사는 최대 256바이트이며, 파일 전체 한도는 307,200바이트다. 읽기 전용 FD·잘못된 사용자 주소·범위 초과가 거부되면 기존 위치와 파일 크기를 보존한다.

게스트 수용 기준은 공개 Picolibc 호출자의 종료값 76이다. 경로명으로 읽는 폰트와 잠금 파일의 수용은 #459에서, 게스트 전체 서버 실행은 #448에서 추적한다. 별도 프로세스가 받은 FD의 오프셋 공유와 물리 보드의 최악 실행 시간은 추가 시험 항목이다.
## Partial-page shrink / 마지막 페이지 축소

After permissions, seals and mapping checks pass, the kernel clears the suffix of the retained page before publishing a smaller file extent. Page-aligned shrink has no retained suffix. The guest caller checks that a mapped shrink returns `EBUSY` and preserves its four readable bytes, and that ordinary shrink followed by growth preserves the prefix and yields zero bytes over the restored extent.

권한·봉인·매핑 검사를 통과한 축소는 새 크기를 기록하기 전에 마지막 페이지의 잘린 부분을 지운다. 게스트 시험은 매핑 중 축소 거부 시 기존 크기와 데이터 유지, 매핑이 없는 파일의 축소·재확장 시 앞부분 보존과 0 채움을 확인한다.
