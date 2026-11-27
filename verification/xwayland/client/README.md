# JNOX Wayland client cross-build candidate

The build has four separately callable phases. Only **prepare** has run. It verified seven pinned inputs, cross-checked the three upstream tarballs against the SHA-256 sections of their Debian source descriptors, checked the graphics runtime artifact hashes against its frozen manifest and applied the client-only patch. Signature authentication of the descriptors was not performed. Neither upstream code generation, a target archive nor a guest roundtrip has been observed for this candidate.

Use a fresh work directory on Linux. Supply the directory emitted by the native Picolibc/Pixman graphics build, with its manifest.json, jnox.ini, original linker inputs and target/lib/{libc,libm,libpixman-1}.a. The measured runtime used 448-graphics-93e41a9/attempts/attempt05/runtime. The original cross file contains absolute paths that must still resolve. Host tools include Python 3.10+, Meson 0.61.2, Ninja, GNU make, GNU tar/patch/binutils, dpkg-deb, pkg-config and the same Clang/LLVM toolchain as the graphics build. Set MESON and PYTHONPATH for the locally extracted Meson if needed.

```sh
tool=verification/xwayland/client
sh "$tool/fetch.sh" "$cache"
python3 "$tool/build.py" prepare "$cache" "$runtime" "$work"
python3 "$tool/build.py" protocols "$cache" "$runtime" "$work"
python3 "$tool/build.py" ffi "$cache" "$runtime" "$work"
python3 "$tool/build.py" client "$cache" "$runtime" "$work"
```

* **protocols** builds the native Wayland 1.23.1 scanner from unmodified upstream source using the pinned Ubuntu amd64 Expat development files in a private directory. The unmodified wayland-protocols 1.44 Meson install generates its actual package metadata and headers. The scanner then produces code and headers from the installed xdg-shell, xwayland-shell and linux-drm-syncobj XML files. The host prefix supplies the executable and scanner pkg-config data for later Xwayland builds.
* **ffi** uses the original libffi 3.4.4 configure script and AArch64 backend with the graphics cross compiler and Picolibc flags. The target pkg-config search excludes host metadata. Every archive member must identify as AArch64 and both ffi_call and ffi_prep_cif must have definitions. A failed bare-metal libffi configure or compile is evidence of a missing prerequisite.
* **client** uses upstream's registry/proxy/queue/message/FD/closure implementations and the generated protocol metadata. The patch drops the server/cursor/EGL build and server-only epoll/credential/accept/remap routines from the separate target source tree. The original client uses exactly three OS helpers: socket creation, FD duplication and receiving ancillary FDs. All three bodies remain upstream. The patch includes the actual transport and utility objects in the installed static archive. An AArch64 symbol check precedes linking the actual wl_display_roundtrip path. Nothing substitutes a successful dummy mutex, socket, clock or FFI call.

The final probe requires a guest loader that supplies a connected Wayland stream in fd 3. A future guest gate must record the display.sync request, callback.done and process exit. It also needs a separate real registry/shm/SCM_RIGHTS exercise to validate surface and descriptor behavior. No guest launcher is part of this preparation.

### Known platform gaps

Inspection of the frozen Picolibc include tree found fcntl.h, while sys/socket.h, sys/un.h, sys/mman.h, pthread.h, poll.h and sys/epoll.h were absent. The SDK has a different include scope. A real port must expose compatible AF_UNIX socket and socklen_t/msghdr/CMSG definitions, preserve SCM_RIGHTS and nonblocking semantics across sendmsg/recvmsg, implement fcntl duplication/close-on-exec, and actually support poll, clocks, pthread mutexes and condition-variable coordination. Inspect additional compiler/linker diagnostics once the gate is granted. The client-only patch removes server epoll and credential checks. It retains the client clock and transport requirements. Transitive linker requirements, errno/TLS behavior and concurrent prepare_read/dispatch still require validation.

### Inputs

The three source versions are Debian's Wayland 1.23.1, wayland-protocols 1.44 and libffi 3.4.4 upstream tarballs. Expat 2.4.7-1ubuntu0.10 is a native build-host dependency. All exact hashes and filenames are in inputs.sha256 and download origins are in fetch.sh. The previous Ubuntu 0.9 APT entry returned 404; the selected 0.10 archive was downloaded directly from the Ubuntu archive.

### 한국어

준비 단계에서는 원본 3종과 DSC의 해시, 호스트 Expat 및 기존 Picolibc 런타임을 확인했다. 다음 단계는 실제 호스트 scanner로 프로토콜 코드를 생성하고 AArch64 libffi와 libwayland-client 정적 라이브러리를 빌드하는 것이다. 현재 동결된 헤더에는 소켓과 poll, pthread 인터페이스가 빠져 있다. 실제 fd 전달과 동기화 구현 및 게스트 왕복은 미검증이다.
