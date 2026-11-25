# IPC-007 first-position results / IPC-007 최초 실행 결과

Frozen candidate `9271a68249103bc6c8674ef5d20d70622c94962b`, tree `a10b3eb56f13a6c60df22ecaf3569f13b13b6a22`; contract `4637f54cafdbcf417a7efdffc9691e931b7f5b95`, issue 379. On single-core AArch64 QEMU virt, all four packaged stream workloads finished. The native shell could not be built. The complete IPC-007 qualification remains open on this revision.

단일 코어 AArch64 QEMU virt에서 FIFO·RR 각각 이름 있는 FIFO와 익명 파이프의 네 원본이 종료했다. 네이티브 셸은 SDK 컴파일에서 실패해 이 커밋의 IPC-007 전체 인수는 보류한다.

| Case / 사례 | Actual first position / 최초 결과 |
|---|---|
| Named FIFO (FIFO/RR) / 이름 있는 FIFO | Per policy, C and Rust each wrote eight records of 512 bytes, and the reader drained 8,192 bytes in 60 positive reads of at most 137 bytes plus one EOF. Origin and sequence bytes, complete payloads, little-endian trailer CRCs and both private writer CRC arrays matched. Arrival order was C 0–7 followed by Rust 0–7 in both boots. The two writer descriptions overlapped. These captures show intact records in that observed order. No alternating-writer dispatch pattern was observed. Exits 61/62/63, high water 2,048 bytes, fifth atomic write −1/EAGAIN (11). |
| Anonymous pair (FIFO/RR) / 익명 쌍 | The writer took two steps, each reader retained the eight bytes `rainfall`, then read zero and saw −1/EPIPE (32) on its reverse write. Exits 64/65. Maxima 4/1 bytes. |
| Resource retirement / 자원 회수 | In all four boots the closing UART snapshot showed zero allocated objects, linked names, readers, writers, descriptor references and queued bytes for all four object slots. |
| Native shell / 네이티브 셸 | With both SDK switches, cat and wc compiled as separate ELFs. Public shell compilation stopped at shell.c line 9, unknown type `int32_t` in the freestanding SDK. Ten requested package commands (two files, missing file, malformed pipeline and unknown second executable under both policies) stopped on the absent shell.elf. Zero shell guest launches, PID console bytes or reap results. GNU wc counted 2 and 76 newlines in the separately retained 26- and 823-byte files. |
| Host controls / 호스트 제어 | Six serialized model tests passed in `host-controls.log`, including descriptor and object exhaustion, exact capacity and atomic boundaries, partial large writes, authorization, generations, last-owner exits, blocked opens, redirection preflight and reference accounting. These cases include host-only observations. Guest negative authorization, large-write and generation/recreate positions were not collected in the four supplied stream applications. |

정책별 이름 있는 FIFO에서 C와 Rust가 각각 512바이트 기록 8개를 보냈고 판독자가 양수 읽기 60회와 EOF 1회로 모두 수신했다. 두 원본에서 C의 0–7번 다음에 Rust의 0–7번이 도착했고 전체 바이트, CRC, 최대 점유량과 최종 참조를 확인했다. 익명 쌍은 실제 `rainfall`, EOF, 역방향 EPIPE를 보였다. 명령별 셸 시도는 기본 ELF의 부재로 패키징에서 중단했다. 두 호스트 파일의 개행 수는 2와 76이다. 접근 권한·세대·큰 쓰기·객체 및 FD 한계의 추가 결과는 호스트 테스트에 해당한다.

The checkout began clean from a local, one-commit shallow fetch. Its tracked source stayed unchanged. Cargo and Python created untracked Cargo.lock and interpreter cache during normal execution. Each app and kernel was built through the public tool. All seven produced apps had executable/read-only/read-write LOADs. ELF hashes, per-position package manifests, build logs, first-position UART, full linked-kernel RAM, tool provenance and invocation records are preserved.

A verifier-side buffered QMP writer delayed the first named FIFO stop and recorded `capture=timeout`. The guest had already emitted the terminal exit lines. The retained helper delivered capabilities, stop and physical-memory save over the same child input channel. QEMU PID 997 and its original RAM survived; the other three positions used the same delivery helper. There were four guest processes total. The two named scheduler-event buffers each reported 82 dropped entries. Their final pipe state, task counters, exit lines and private-memory result arrays are present. Event-by-event reconstruction of all scheduling decisions is unavailable for those two positions.

검증기의 QMP 명령 버퍼링 때문에 첫 FIFO 게스트의 종료 후 RAM 저장이 지연되어 자동 관찰값은 timeout이다. 원래 QEMU PID 997의 동일 입력 채널로 정지와 메모리 저장 명령을 전달했다. 첫 실행의 종료 UART와 RAM을 보존했고 재부팅은 없었다. 두 이름 있는 FIFO 원본의 스케줄 이벤트는 각 82개가 유실되었다. 최종 파이프 상태, PID 종료 기록, 사적 메모리 값은 남아 있다.

For offline replay from the extracted directory, run `sha256sum -c SHA256SUMS`, then `python3 replay.py > /tmp/379-9271a68-offline.json` and compare with `report.json`. The reducer traverses the three actual AArch64 table levels for each private result page. It checks user-write permissions, distinct physical pages, manifest/ELF/UART identities, byte-level oracles, final references and four simulated evidence mutations per named position. These deliberately changed byte arrays occur only inside the reducer. The original retained RAM is read-only input.

압축을 푼 뒤 SHA256SUMS를 검증하고 replay.py 결과와 report.json을 비교한다. 판독기는 각 프로세스 사적 결과의 실제 3단계 페이지 테이블과 권한을 따라 원본 RAM을 읽는다. 원본 바이트, 실제 반환값, PID별 ELF 식별자 및 최종 상태를 확인한다.
