# C permit codec on AArch64 JNOX / AArch64 JNOX의 C 허가 코덱

A freestanding C application can encode and decode the native 44-byte permit with the same Rust library as the native Rust sender and receiver. Build with `sh sdk/native-aarch64/build.sh --permit-codec APP.c APP.elf`. The public C interface takes caller-owned private storage, checks the supplied length and writes the destination only on success. The codec interprets bytes. The channel retains responsibility for authenticated endpoints, freshness, sequence progression, COMMIT and the output. REQ-019 · DES-010 · IMP-311 · VER-019 · EVD-311. [C example and API guide](../../examples/native-permit-codec/README.md) · [Issue #405](https://github.com/Romanof123/JNOX/issues/405)

C 앱이 기존 Rust 송수신 앱과 같은 Rust 라이브러리로 44바이트 허가 프레임을 생성·판독할 수 있다. C 호출자는 사설 버퍼와 길이를 넘긴다. 성공 시에만 목적지가 갱신된다. 송수신자 권한, 시간 유효성, 순번 진행, COMMIT 및 출력 제어는 기존 채널이 처리한다.

| Run / 실행 | Observation / 관측 |
| --- | --- |
| First documented C example on the [original implementation](https://github.com/Romanof123/JNOX/commit/d8f4da72b438acb89a8ba642a0627fd3ca8ffa0d) | The ordinary loader returned `FAULT 5` before process entry. Linker garbage collection removed the last writable section of this stack-only app. Original ELF has RX and R load segments. / 스택만 쓰는 예제에서 쓰기 구역이 사라져 앱 시작 전에 적재가 거부됐다. |
| Same example on the [corrected SDK](https://github.com/Romanof123/JNOX/commit/438e3771f9bd4cc02b379407acdb6e506c4f2892) | The link retained the native errno fallback. RX, R and RW load segments are present. A fresh ordinary JNOX guest reported `EXIT 0` after the codec round trip. / SDK 링크를 수정해 코드·읽기·쓰기 구역을 갖춘 동일 예제가 정상 종료했다. |
| Independent C caller on the corrected SDK | A separate native JNOX guest reported `EXIT 0`. The verifier found all 48 expected result records in the caller’s private RAM through the active user page tables, and authenticated the loaded ELF and UART. / 독립 C 앱이 종료 코드 0을 반환했고, 실제 사용자 페이지 테이블을 따라 사설 RAM의 48건과 원본 UART·ELF를 대조했다. |

Four independent frames of differing generation, sequence, issue tick and permit produced eight matching encode/decode results. Forty invalid cases covered bad lengths, CRC, version, type, channel, data ID, validity, reserved bytes, generation, sequence, null arguments and precedence among errors. The independent reference checked status and every byte of guarded destination storage on all 48 cases. Native target was single-vCPU Cortex-A72 QEMU virt. A direct build comparison also reproduced the pre-change ordinary telemetry and publish/subscribe example ELFs byte for byte without the codec flag.

독립 기준 프레임 4개의 인코딩·디코딩 8건이 기대 바이트와 일치했다. 잘못된 길이·CRC·필드·인자와 오류 우선순위 40건에서도 기대 상태와 목적지 바이트가 모두 일치했다. 기존 일반 C 예제와 발행·구독 예제의 선택 옵션을 유지한 빌드에서도 변경 전후 ELF 바이트가 같았다. 측정 대상은 QEMU virt의 단일 Cortex-A72이며 세 프로세스의 정각 만료는 [#401](https://github.com/Romanof123/JNOX/issues/401)에서 추적한다.

## Original evidence / 원본 재현

The [three original captures, C caller and offline verifier](native-permit-c-codec-first.tar.xz) have SHA-256 `41a6d797603ba90825b7f367f060037c7190cdfc4f2c429317eb992223234306`. Extract the packet, then run `bash replay.sh` in its root with Python 3 and GNU AArch64 binutils. The replay checks each capture receipt and segment layout, then matches the independent RAM-decoded report to the recorded JSON. The packet includes original application and kernel ELFs, raw terminal RAM and UART, and the independent caller, fixture generator and expected values.

[원본 부팅 및 검증 도구](native-permit-c-codec-first.tar.xz)를 풀고 루트에서 `bash replay.sh`를 실행한다. 실제 첫 실패와 수정 후 두 부팅의 기록, 커널에 포함된 앱 이미지, 종료 RAM·UART, 독립 C 호출자와 기준값을 함께 보존했다.
