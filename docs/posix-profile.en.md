# Native POSIX source API profile 0.1

[한국어](posix-profile.md) · CHG-078 · [Extended user APIs · #8](https://github.com/Romanof123/JNOX/issues/8)

**Planned for the AArch64 extended profile.** Version 0.1 selects source interfaces for C programs rebuilt with the J-NOX SDK. Each function starts with status *planned*. Mark it verified only for a named SDK, image, configuration and target-user-mode result. A linked EL1 task or a host libc test exercises a different interface. Certification and a full libc would require separate scope and evidence. [POSIX.1-2024, Issue 8](https://standards.ieee.org/ieee/1003.1/7700/) · [Current platform evidence](platform-checklist.en.md)

## Selected functions and target checks

Compile separate EL0 applications with the SDK headers `<fcntl.h>`, `<unistd.h>`, `<spawn.h>`, `<sys/wait.h>`, `<time.h>`, `<pthread.h>`, `<signal.h>`, `<poll.h>` and `<sys/socket.h>`, plus the selected address-family headers. Exercise their libc entry points on the target. Record a result for each named function, the tested flags/options, return value, any specified error, observed side effects and cleanup. For ordinary system-style calls, check `errno` after the documented failure return. The selected `pthread_*`, `posix_spawn*`, `sigwait` and `clock_nanosleep` functions report an error number directly. Successful calls need no `errno` check. Induce concurrent failures in two threads and check that each sees its own `errno`.

| Group and trace | Selected interfaces | Observable target case |
| --- | --- | --- |
| Files and descriptors · VER-014/020 | `open`, `read`, `write`, `close`, `dup2`, `fcntl` | Create/truncate and reopen a permitted file. Verify exact bytes, offsets shared by duplicates, short I/O and EOF. Use an absent pathname for `ENOENT`, denied read for `EACCES` and an invalid descriptor for `EBADF`. Exercise `F_GETFL/F_SETFL` for `O_NONBLOCK` and `F_GETFD/F_SETFD` for `FD_CLOEXEC`. |
| Pipes and children · VER-014/020 | `pipe`, `posix_spawn`, `posix_spawn_file_actions_init`, `posix_spawn_file_actions_destroy`, `posix_spawn_file_actions_adddup2`, `posix_spawn_file_actions_addclose`, `waitpid`, `_exit` | Apply ordered file actions to put the two pipe ends on child stdin/stdout. Close surplus ends in both children and the parent. Drain buffered bytes, then observe zero-byte read after the last writer closes. Reap both PIDs, decode statuses with `WIFEXITED/WEXITSTATUS`, then expect `ECHILD` on a second wait. Verify close-on-exec behavior in the child. |
| Clocks · VER-014 | `clock_getres`, `clock_gettime`, `nanosleep`, `clock_nanosleep` | Read resolution and nondecreasing monotonic time, and read initialized realtime. Step realtime during an absolute monotonic sleep and verify its original deadline. Interrupt a relative sleep with a caught signal and check the remaining interval. For invalid nanoseconds, check `nanosleep` returns −1/`EINVAL` and `clock_nanosleep` returns `EINVAL`. [Time contract](system-time-plan.en.md) |
| Threads · VER-005/014 | `pthread_create`, `pthread_join`, `pthread_mutex_init`, `pthread_mutex_destroy`, `pthread_mutex_lock`, `pthread_mutex_trylock`, `pthread_mutex_unlock`, `pthread_cond_init`, `pthread_cond_destroy`, `pthread_cond_wait`, `pthread_cond_timedwait`, `pthread_cond_signal`, `pthread_cond_broadcast` | Use process-private objects and joinable threads. Check returned thread data and direct `EBUSY` when a peer holds the mutex. Wait in a predicate loop. Signal one waiter and broadcast to multiple waiters. Verify the mutex is held again after wake or direct `ETIMEDOUT`, then destroy quiescent objects. |
| Basic signals · VER-005/014 | `sigemptyset`, `sigaddset`, `sigaction`, `pthread_sigmask`, `sigwait`, `getpid`, `kill` | Block `SIGUSR1` before starting threads, send it to the process and consume the pending signal once with `sigwait`. In another case deliver an unblocked signal to a handler. With `SIGPIPE` ignored, writing to a pipe with no reader must fail with `EPIPE`. Observe default `SIGTERM` via `waitpid` and `WIFSIGNALED/WTERMSIG`. |
| Readiness and sockets · VER-006/014 | `poll`, `socket`, `bind`, `listen`, `accept`, `connect`, `send`, `recv`, `sendto`, `recvfrom`, `shutdown`, `close` | Use AF_UNIX streams and both IPv4/IPv6 TCP streams and UDP datagrams. Check `poll` timeout, readable input, closed, nonnegative fd (`POLLNVAL`), partial stream transfer, drained peer EOF and a nonblocking would-block result. Preserve UDP message boundaries. Run multiple clients, exercise close under load and recover descriptors/buffers after saturation. |

For resource tests pin the manifest limits for processes, threads, descriptors, pipe storage, sockets, queues and outstanding waits. Exhaust one at a time, check the specified error or bounded backpressure, release the resource and repeat successfully. For example, reaching the process fd limit makes `open` fail with `EMFILE`. Record limits and reclamation observations per configuration. Native IPC ownership, cancellation and peer-death rules remain in REQ-024. The PM links scheduling acceptance in #21 before opening IPC implementation in #24. [IPC profiles](ipc-mechanisms.en.md) · [Product sequencing](product-plan.en.md)

## Application acceptance and scope

- **Native C · VER-020.** Build external static `cat` and `wc` with the SDK. In the target shell run `cat sample.txt | wc -l` for bytes `61 0a 62 0a`. Require count 2, empty stderr, EOF and status 0 for each child. Missing input must produce a producer diagnostic and nonzero status while the consumer still receives EOF and exits.
- **Native Rust · VER-022.** A separate Cargo project builds a static `core` application. Use the public safe SDK to copy the named file to stdout and return status 0. Verify byte equality, empty stderr, and denial with nonzero status and no contents disclosed for a forbidden file. Publish `alloc` and `std` status separately.
- **Prebuilt Linux ELF · VER-021.** Use the verification plan's pinned Debian arm64 static BusyBox unchanged, with the Linux guest disabled. Compare `wc -l sample.txt` output, stderr and status against reference Linux for success and access denial. Exercise raw Linux `-ENOSYS` and reject an incompatible ISA or interpreter before entry. [Exact fixtures, digests and comparison rules](verification-plan.en.md)

Process-shared synchronization, robust mutexes, real-time queued signals, `fork`, dynamic linking and full shell/utility coverage are deferred for separate profile decisions. Publish omitted options and unsupported flags in the SDK. A header, exported symbol or linked executable alone leaves target verification pending. [Requirements](requirements.en.md) · [User services and ABI design](architecture.en.md)

## CHG-085 · native static executable layout

The first native AArch64 image contract accepts little-endian ELF64 ET_EXEC, version 1, machine 183, System V identification with ABI version zero and AArch64 flags zero. The SDK supplies native startup code and syscall conventions. ELF identification alone cannot establish the application ABI. The planner accepts immutable file bytes and a caller-provided page-aligned, half-open user VA window. The reference SDK window is [0x00400000,0x00800000), with 4096-byte pages. Use an explicit linker PHDRS table, for example RX code at 0x00400000 and any writable data on a separate page.

Accept 1–8 nonempty PT_LOADs in ascending VA order and at most 16 program headers. Accept R, RX or RW only. Require aligned virtual/file starts, power-of-two alignment at least one page, ELF offset/address congruence, file size ≤ memory size, bounded file data, checked VA arithmetic, rounded mapped pages wholly inside the window and disjoint page ranges. Require a 4-byte-aligned entry and a complete first instruction in file-backed RX bytes. Only PT_NULL and an optional single empty non-executable PT_GNU_STACK are additional supported headers. Reject all others, including interpreter, dynamic and TLS. The plan ignores section headers, symbols and physical addresses. Copy each declared file interval, zero its memory tail and any exposed page padding before mapping with final permissions. A failed parse returns no partial plan.

The host inspector and future kernel mapper use the same allocation-free Rust parser. Validate an SDK-built C image against its link map and record bounded format/range controls. Actual mapping, stack, EL0 entry, syscall handling and application results require separate implementation and evidence. [REQ-008/014](requirements.en.md) · [DES-004/005](architecture.en.md) · [VER-008/014](verification-plan.en.md)

Before launch the loader must independently authenticate the executable and its native ABI identity, for example with a trusted package manifest binding the image digest and ABI version. A hosted comparison image passing these layout checks gains no native execution authorization.

### Inspect an SDK image

From the repository root with Rust installed, compile the inspector once and supply the ELF produced by the SDK linker. The report gives the entry, permissions, copy and BSS lengths, and page ends. Compare those fields with the SDK link map. The SDK v1 startup convention, allocated JNOX note and syscall ABI are separate package checks. The inspector neither validates the note nor launches the application.

```sh
rustc --edition=2024 tools/native_elf_plan.rs -o native_elf_plan
./native_elf_plan path/to/application.elf
rustc --edition=2024 --test kernel/native/lib.rs -o native_elf_tests
./native_elf_tests
```
