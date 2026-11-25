# Native pipe and named FIFO SDK

Use the opt-in SDK compiler switch with a native package that enables the pipe service. The Rust application also enables the pipes Cargo feature of jnox-native.

~~~sh
sdk/native-aarch64/build.sh --pipes writer.c writer.elf
sdk/native-aarch64/build-rust.sh --pipes reader/Cargo.toml reader.elf
~~~

C uses unistd.h for pipe, pipe2, dup, dup2, read, write, close and unlink; fcntl.h for open, fcntl and flags; sys/stat.h for mkfifo; and limits.h for PIPE_BUF. The first pipe endpoint reads and the second writes. Each pipe has 2,048 bytes of capacity and an atomic-write limit of 512 bytes. Larger writes can complete partially. A positive-length write with no readers returns -1 with EPIPE; this profile does not deliver SIGPIPE. Read returns zero after the last writer closes and committed bytes have drained. Check every signed result, including short reads and short writes.

Access modes are O_RDONLY and O_WRONLY. O_NONBLOCK belongs to the open description and is shared by duplicates. FD_CLOEXEC belongs to the individual descriptor. F_GETFD, F_SETFD, F_GETFL and F_SETFL are supported. Automatic allocation uses 3 through 10. dup2 may target 0 through 10; the kernel limits each process to eight live pipe references. Known live pipe descriptors, including redirected standard I/O, use pipe read/write/close. A wrong-direction pipe error is returned to the caller. An unowned descriptor follows the ordinary native SDK path unless that specific pipe descriptor was explicitly closed. A subsequently opened pipe or ordinary descriptor at that number can be used again.

Named FIFO permissions come from the package grants. mkfifo accepts the normal C mode argument; ownership and directional grants come from that manifest. The kernel receives counted 1-to-31-byte names. A refused FIFO open propagates its error; a missing FIFO name permits ordinary open to try the same path. The Rust open_fifo function addresses only the FIFO namespace. Unlink removes the name; already open endpoints refer to the old generation until closed. The SDK implements neither a filesystem nor shell process launch.

Rust functions under jnox_native::pipes return Result with negative errno. pipe2 returns two OwnedFd values; open_fifo takes a byte slice, Access::Read or Access::Write, and flags. OwnedFd reads/writes slices, duplicates with dup or dup2, exposes descriptor and status flags, and closes on drop. Explicit close consumes the value. For an exclusively owned inherited endpoint, unsafe OwnedFd::from_raw_fd transfers closing responsibility to Rust. into_raw_fd transfers it back without closing.

With the local-socket client sources installed, both SDK switches may be passed in either order. The socket linker wrappers own descriptors 64 through 67 and delegate other read/write/close calls to io.c. Pipe mode adds no duplicate linker wrappers. Ordinary SDK builds without --pipes compile the original I/O branches.
