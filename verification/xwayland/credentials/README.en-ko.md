# Immutable service account / 불변 서비스 계정

CHG-340 · REQ-015/024 · DES-009/013 · VER-015/024. Xwayland 24.1.6 `os/utils.c:PrivsElevated` compares the real and effective group and user IDs. The selected configuration then tries `seteuid(0)` to detect a saved privileged identity. `XkbProcessArguments` also checks whether the user IDs differ. [Native interface tracking #471](https://github.com/Romanof123/JNOX/issues/471)

The fixed-priority process package has one service account for its statically linked programs, including a shell's packaged children. Pass `--service-account 4101:5101` to `tools/build_native_el0.py` with the usual native FIFO/RR process arguments. Each ID must be in 1–65534. The default account is 65534:65534. The build records the pair in `manifest.json` and embeds the validated numbers in the kernel. The privileged packager controls both values. All processes in the package share this account and keep their own capability grants.

| Public operation | Kernel behavior in the selected profile |
| --- | --- |
| `getuid()`, `geteuid()` | Return the package UID. Real and effective IDs are equal throughout the process lifetime. |
| `getgid()`, `getegid()` | Return the package GID. The profile has a single primary group. |
| `seteuid(uid)` | The kernel accepts the current UID and preserves it. Root and other valid account IDs return `EPERM`. The reserved all-ones ID returns `EINVAL`. |

SVC 203 queries the user (selector 0) or group (1), or requests an effective UID (2). Queries take zero argument words. For a request, x1 is the full-width proposed UID and x2 is reserved zero. The kernel validates this value before comparing IDs. Getters in the Picolibc adapter terminate with 126 if run against a kernel that cannot supply a valid identity. This prevents paired invalid IDs from silently passing the upstream privilege comparison.

Privileges remain attached to kernel-enforced process slots, private descriptor tables and package capabilities. Services sharing this UID cannot use it to acquire another process's descriptors or device grants. Xauthority admission and the filesystem rights model have independent checks. This profile defines no account database, multi-user login, privilege transitions, supplementary groups or UNIX ownership enforcement on named files.

For the guest check, compile `guest.c` with the native Picolibc graphics flags and link against `libjnox-wayland-os.a`. Run the normal FIFO process package with the default account. The expected application exit is 84. To test a declared package account, compile the caller with `-DEXPECT_UID=4101 -DEXPECT_GID=5101`, package with `--service-account 4101:5101` and require the same exit. This checks distinct user/group values, the four lookups, root/foreign/sentinel denials and unchanged IDs after both rejection and self-assignment.

FIFO/RR 프로세스 패키지는 UID와 GID 각 1–65534를 지정할 수 있다. 기본 계정은 65534:65534이며, 지정값은 매니페스트와 커널에 함께 기록한다. 같은 패키지에 포함된 셸 자식도 이 계정을 유지한다. `seteuid`는 현재 UID만 허용하고 루트 또는 다른 유효 UID에는 `EPERM`, 예약 값에는 `EINVAL`을 반환한다. 커널이 신원을 제공할 수 없으면 조회 호출자는 126으로 종료한다.

프로세스별 장치·IPC·파일 디스크립터 권한과 Xauthority 인증은 기존 경로에서 검사한다. 게스트 시험의 기대 종료값은 기본 계정과 명시적으로 지정한 4101:5101 모두 84다. 서로 다른 UID/GID, 네 가지 조회, 권한 변경 거부와 거부 후 값 보존을 확인한다. 이름 있는 파일에 대한 소유권 검사는 파일시스템 이슈에서 별도로 추적한다. [파일시스템 #459](https://github.com/Romanof123/JNOX/issues/459)
