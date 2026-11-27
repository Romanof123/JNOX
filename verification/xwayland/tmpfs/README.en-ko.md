# Volatile pathname storage / 휘발성 경로 파일

Native Rust applications can open this mount through an opt-in owned file handle. [Rust SDK and guest acceptance / Rust SDK와 게스트 시험](rust-sdk.en-ko.md)

CAP-007 · REQ-010/011/014 · CHG-340 · REQ-015/024 · VER-015/024. [Filesystem #459](https://github.com/Romanof123/JNOX/issues/459) · [Xwayland OS interfaces #471](https://github.com/Romanof123/JNOX/issues/471)

The Rust EL0 broker hosts a volatile `/tmp` mount when built with Cargo feature `tmpfs` and granted clients at packaging time. The kernel's SVC 204 discloses the trusted endpoint to an authorized caller and the grant mask only to the broker. It can validate a client's own bounded I/O buffer. Paths, file data, permissions, descriptors, and mutations live in the user service. The existing IPC sender slot and process-exit notification identify each client. The service publishes a mutation after the kernel accepts its reply token. [Rust service](../../../services/socket/src/tmpfs.rs) · [Picolibc API](../../../sdk/native-aarch64/graphics/os/tmpfs.c)

| Contract | Selected AArch64 FIFO/RR profile |
| --- | --- |
| Mount and policy | Read-only root and hierarchical `/tmp`. Paths permit 63 bytes, with 1–31 bytes in each component. The service rejects repeated separators, relative paths, and dot traversal. `--tmpfs SLOT:ro` grants lookup/read. `--tmpfs SLOT:rw` also grants namespace changes. The role must have an IPC route to the AF_UNIX broker. Both roles use the immutable package service account. POSIX owner bits gate subsequent opens and directory traversal. Package grants cap each request. |
| Capacity | Four inodes of 1,024 bytes each. Eight file descriptors per client, numbered 32–39. Each read or write transfers at most 64 bytes. A full file or inode table reports ENOSPC. An exhausted per-client descriptor table reports EMFILE. |
| Lifetime | Opens create distinct positions. Duplicates share one position. Unlink removes the name immediately; open descriptions keep the previous inode alive. Exit retires that client's FDs. Named data lasts for the broker lifetime. Its reinitialization or a cold boot starts an empty volume. |
| Metadata | Stable inode identity, logical size, mode, link count, package UID/GID and three actual service timestamps. The package boot UTC source (SVC 207) supplies timestamps. Without an epoch, file I/O remains available and metadata queries fail with ENODATA. Seeded time accuracy depends on the declared epoch. |
| Directories and links | Six directory slots include root and `/tmp`. Twelve directory entries include `/tmp`. Hard links refer to the same file inode. Rename switches names atomically and preserves existing FDs to the replaced inode. Directory search needs owner execute, and modifying entries needs owner write and execute. Sticky directories carry the actual package UID. |
| Synchronization | Writes and directory changes reach the live broker before returning success. A named live FD returns ENOTSUP from `fsync` because this mount has no stable writeback device. A closed named FD returns EBADF. Persistent block flush and power-loss recovery remain tracked under #459. |

Build `services/socket/Cargo.toml` with `sdk/native-aarch64/build-rust.sh --features tmpfs` and compile [guest.c](guest.c) against the graphics Picolibc overlay. For the two-role FIFO gate, package the caller in slot 0 and the broker in slot 1 with `--peer-arg 1 --ipc-route 0:1 --socket-service 1 --tmpfs 0:rw --boot-utc-seconds 1700000000`. The caller's expected exit is 88. The default mode checks cold-empty state, independent opens and aliases, sparse writes, mode/mask, real metadata, unlink while open, inode reuse and both capacity bounds. Repeat a cold boot of the same package to check volatile loss. Use caller argument `ro` with a read-only grant and `unset` with an rw grant and no UTC seed.

For a separate owner and consumer, give the writer and reader distinct client slots with priorities that let the writer finish before the reader opens the name. Use the same caller ELF with arguments `writer` and `reader`, rw and ro grants respectively. The reader observes the writer's named data and the service denies unlink. A routed ungranted role can run the `denied` mode while another role holds the volume grant. Record actual guest exits before promoting each acceptance result.

The [pathname caller](pathops.c) has expected exit 89 in the same seeded rw package, and also has a `ro` mode. It exercises the Xtrans-shaped `/tmp/.X11-unix` directory and the server's temporary-lock, fchmod, hard-link and unlink sequence. It checks directory search denial, stable file and directory identities, hard-link counts, open-but-replaced lifetime, denied cycles, nonempty directory replacement, and directory-entry exhaustion. The Rust socket broker still has its own AF_UNIX address registry. A directory inode alone does not supply filesystem metadata for a bound socket endpoint. An upstream build requiring root ownership for a sticky directory also needs a compatible deployment policy. The package account stays nonroot.

## Serial guest gates / 직렬 게스트 시험

From the source checkout, set `runtime` to the verified Picolibc graphics runtime and `out` to a new absolute directory for one selection. Run under the shared build/guest lock. Keep `CARGO_BUILD_JOBS=1` and the cargo target path in the selected output directory. The build entry point packages the actual native ELF with the feature-enabled Rust broker.

```sh
mkdir "$out"
export CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$out/target"
sh sdk/native-aarch64/build-rust.sh --features tmpfs services/socket/Cargo.toml "$out/broker.elf"
python3 examples/native-xwayland-shm/graphics_dup_gate.py \
    "$runtime" "$out" "$(command -v qemu-system-aarch64)" --tmpfs-roles
```

| Selection | Required terminal rows |
| --- | --- |
| `--tmpfs-paths` | RW pathname caller 89; broker 0. |
| `--tmpfs-paths-ro` | RO pathname caller 89; broker 0. |
| `--tmpfs-roles` | One FIFO guest: PID 0 writer 88, PID 1 reader 88, PID 2 ungranted caller 88, PID 3 broker 0. Writer publishes and leaves `/tmp/share`; reader verifies its data and is denied unlink. The third client has a real broker route and is denied mount access. |
| `--tmpfs-cold` | The same ELF package boots twice in separate QEMU processes, one at a time. On each boot PID 0 first requires ENOENT for `/tmp/cold-marker`, then creates, writes, reopens, reads and stats it without unlinking. Both boots require caller 88 and broker 0. |

Each selection writes numbered capture directories containing the launch command, UART and stderr. After all required exits are observed, `tmpfs-result.json` records the executable hashes and each PID, role and exit status. The cold gate checks kernel identity before and after each boot. The broker in these packages finishes after kernel exit notifications for every routed caller. A missing broker row remains a failed gate.

각 모드는 새 출력 디렉터리에서 한 번씩 직렬로 실행한다. 네 역할 시험은 같은 커널에서 쓰기·읽기·권한 없는 앱과 브로커의 종료값을 각각 검사한다. 재부팅 시험은 한 번 빌드한 동일 커널을 두 개의 독립적인 QEMU 프로세스에서 차례대로 부팅한다. 첫 부팅 끝까지 남은 파일이 다음 부팅 시작 때 없는지 앱이 직접 확인하며, 두 UART와 프로세스별 결과를 별도로 보존한다.

Rust 사용자 서비스에 `/tmp`를 둔다. 패키지가 클라이언트 슬롯별로 읽기 또는 읽기·쓰기 권한을 지정하며 커널이 IPC 발신자와 종료를 확인한다. 독립적인 open은 별도 위치를 가지며 복제 FD는 위치를 공유한다. 하드 링크와 열린 FD는 같은 inode를 참조하고 rename은 기존 FD를 유지한다. 디렉터리 탐색은 소유자의 실행 권한을, 이름을 변경하려면 상위 디렉터리의 쓰기·실행 권한을 요구한다. 용량과 파일 개수가 고정되어 있고 재부팅하면 내용은 사라진다. 공개 stat 계열은 패키지 UID/GID와 UTC 기준을 사용하며 기준이 없으면 ENODATA를 반환한다. 이 RAM 볼륨은 영속 동기화 요청에 ENOTSUP를 반환한다. AF_UNIX 주소와 파일시스템 소켓 inode 연동, 영속 블록 저장소와 전체 서버 기동은 별도 게스트 수용 항목이다.
