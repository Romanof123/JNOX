# Role-scoped boot resource acceptance · 부팅 리소스 역할별 검증

Run from a Linux checkout with the ordinary native AArch64 packaging toolchain and QEMU system AArch64. Supply a new output directory.

```sh
python3 verification/xwayland/authority/resource-gate.py /path/to/new-result
```

Two images contain the same ordinary user probe in every role. The five-active image exposes SVC 199. The four-role image checks ENOSYS, even when the emulator supplies both files. Each boot generates new private inputs and attaches them via QEMU fw_cfg. The server and client FamilyLocal records have distinguishable public node names and freshly generated opaque data. Their different opaque data is intentional for transport isolation. These fixtures test the EL1 byte provider. A server authentication verdict requires a separate X11 connection attempt.

The output has one JSON result per boot. Exit codes 40/41 identify own/crossed role markers, 42/43 report absent/invalid firmware files, 44 is denied and 45 is unimplemented. Codes 46/47 confirm the minimum and maximum lengths. For valid data the probe checks repeated size/copy consistency, short-buffer rejection, unchanged sentinels, a null destination and, for records larger than eight bytes, a destination crossing the ordinary stack’s upper guard. Every process emits only its final numeric exit status. Generated credentials live in mode-0600 files inside a temporary mode-0700 directory and are removed after each boot.

각 부팅에서 새 비공개 입력을 생성한다. 전용 역할의 성공, 다른 역할의 거부, 크기 경계와 잘못된 주소, 쓰기 실패 시 버퍼 보존, 일반 네 역할 프로필에서의 호출 비활성화를 검사한다. 공개 가능한 결과는 테스트 이름, 프로세스 종료 코드, 통과 여부뿐이다.

## Recorded Ubuntu results · Ubuntu 실행 기록

Nine AArch64 QEMU boots passed on the first attempt. The positive and crossed cases identified the caller’s own firmware key by its nonsecret record marker. The four-role image returned ENOSYS for every caller even with both files attached. The full case table contains only the case name, per-PID exit codes and acceptance flag. [Nine boot verdicts](resource-exits.json) · [EL1 resource contract](../../../docs/native-display-authority.en-ko.md) · [Issue #465](https://github.com/Romanof123/JNOX/issues/465)

The host broker tests passed 4/4 in the default configuration and 5/5 with display-five-active enabled. Run both Rust suites using the [five-role profile instructions](../five-active.md), then run the resource gate above from a clean checkout. Ubuntu requires Python 3, QEMU system AArch64 with virt fw_cfg support, the native Rust target, Clang/LLD and the repository packaging dependencies. The host program creates the four-role and five-role images once and reuses them across the cases.

실제 AArch64 게스트를 아홉 차례 부팅했고 모든 케이스가 첫 실행에서 통과했다. 기본 브로커 테스트 4개와 다섯 역할 브로커 테스트 5개도 Ubuntu에서 모두 통과했다. 정상 및 교차 입력에서는 ELF가 비밀이 아닌 노드 표식을 읽어 자기 역할에 공급된 레코드를 구별했다. 실제 X 서버가 클라이언트의 인증을 허용하거나 거부하는지는 별도의 연결 시험에서 확인해야 한다. [케이스별 종료 코드](resource-exits.json)

EVD-346 covers the role-scoped transport and grants for CHG-346 / IMP-346 under VER-011.
