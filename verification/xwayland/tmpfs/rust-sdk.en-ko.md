# Rust SDK for native /tmp / 네이티브 휘발성 파일의 Rust SDK

CAP-007 · REQ-010/011/014 · VER-010/011/014. The opt-in `jnox-native` feature `tmpfs` exposes `tmpfs::File`, `SeekFrom` and `unlink` to a native AArch64 `no_std` application. Build the app with `sdk/native-aarch64/build-rust.sh --tmpfs`. Link-time selection includes the native client bridge. The package must grant file access and route the client's IPC to the mounted EL0 service. [Filesystem #459](https://github.com/Romanof123/JNOX/issues/459) · [Shared mount and capacities](README.en-ko.md)

```rust
use jnox_native::tmpfs::{File, SeekFrom, unlink};

fn example() -> Result<(), i32> {
    let path = b"/tmp/message";
    let mut file = File::create_new(path)?;
    if file.write(b"OK")? != 2 { return Err(-5); }
    file.seek(SeekFrom::Start(0))?;
    let mut text = [0; 2];
    if file.read(&mut text)? != 2 || &text != b"OK" { return Err(-5); }
    file.close()?;
    unlink(path)
}
```

`File::create_new` requests exclusive creation with owner read/write permissions (0600). `File::open` opens for reading and `open_read_write` reopens without truncation. Path arguments are byte slices. The service validates components and package grants. A file owns its descriptor, passes slice lengths to the existing checked IPC transport, and attempts close on drop. An explicit `close` reports failure and keeps the handle available for retry. The broker also reclaims descriptors on client exit. Reads and writes return the actual byte count, at most 64 per call. Errors are native negative errno values. [Rust API](../../../sdk/native-aarch64/rust/src/tmpfs.rs)

Run the four-role acceptance from the repository root with a fresh output path. The build requires the native Rust target, Clang/LLD and the usual kernel packaging and QEMU tools. All builds and one guest boot run under the same lock.

```sh
flock "$JNOX_LAB_ROOT/run.lock" sh verification/xwayland/tmpfs/rust-guest.sh "$NEW_OUT"
```

The writer checks partial I/O, independent reopen, seek, error preservation, unlink while open and repeated descriptor release on drop. It publishes six bytes to a second file. A reader with a read-only grant checks the bytes and denied mutations. A routed client with no mount grant checks access denial. Expected exits are 90 for each Rust client and 0 for the broker. The runner retains each terminal row and the ELF identities in `tmpfs-result.json`. [Rust guest](rust-guest/src/main.rs)

네이티브 Rust 앱은 Cargo의 `tmpfs` 기능과 빌드 옵션 `--tmpfs`를 선택한다. 파일 핸들은 디스크립터를 소유하며 `Drop`에서 반환한다. 명시적으로 닫을 때 오류가 있으면 같은 핸들로 재시도할 수 있다. C와 Rust가 동일한 권한 검사와 IPC 전송 코드를 사용한다. 쓰기 앱은 파일 생성·부분 전송·탐색·재열기·삭제를 확인하고, 읽기 전용 앱과 권한이 없는 앱은 같은 게스트에서 허용·거부 결과를 검증한다.
