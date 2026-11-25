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

## Native shell catalog profile

For the shell and each utility, build with --native-shell. This enables --pipes and the read-only boot-file library. The kernel package must supply the native shell catalog and map the 8,192-byte JNOXROM1 image at 0x01800000 in each participating process. This option relies on that mapping.

~~~sh
sdk/native-aarch64/build.sh --native-shell shell.c shell.elf
sdk/native-aarch64/build.sh --native-shell cat.c cat.elf
sdk/native-aarch64/build.sh --native-shell wc.c wc.elf
~~~

Include jnox/process.h. jnox_spawn(program, argv, stdin_fd, stdout_fd) passes the NUL-terminated program name, null-terminated argv vector, and signed redirections to native service 50. Pass -1 for an ordinary standard stream. Success returns the child's PID. jnox_waitpid(pid, &status) uses native service 51 and returns that same PID on success, with the signed native exit code in status. A null status pointer discards the code. These functions use -1 and positive errno on failure.

After checking the named FIFO service, ordinary open finds an exact boot-file name of at most 31 bytes. Each process owns four independent read-only descriptions in slots 16 through 19. Reads copy the smaller of the requested count and remaining bytes, advance that description's cursor, and return zero at EOF. Close releases the description. A malformed header, out-of-bounds entry or ambiguous requested name is rejected before opening the file. Boot descriptors stay local to the opening process.

After pipe ownership and closed-descriptor checks, ordinary stdout and stderr writes use native service 4. The shell runtime validates the user span and supplies console output. An inherited pipe at standard output uses the pipe service instead. Neither the shell SDK nor the file table supplies a terminal command or executes a pipeline on its own.
