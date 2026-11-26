# Native permit and task-policy integration / 네이티브 허가 채널·태스크 정책 통합 검증

Nine serial QEMU guests exercised the public merge of the permit/loan changes and main's native task policy at commit `52e4a8b23897801a418da23cac18fefca4cea35b`, tree `aa185fdf2ee73cd1633153ab4ce7ddd6577bf321`. All nine built through the normal packager from a clean Linux checkout and reached their terminal markers. Both healthy fused safety runs exceeded the 1 ms controller-phase limit. Timing admission remains refused.

공개 통합 커밋의 기본 패키징 경로에서 게스트 9개를 순차 빌드·부팅해 예상 종료를 모두 확인했다. 보호 채널 정상 사례의 FIFO와 RR에서 각각 제어 위상 두 개가 1 ms를 넘었다. 두 정책의 시간 승인은 계속 거부한다.

## Native task policy / 네이티브 태스크 정책

The five-app, six-task deployment used four C ELFs and a Rust heartbeat ELF. With supervision enabled, heavy reached its independent deadline and then its watchdog while continuously running in EL0. Both observations came through PPI30. Equality at the 15 ms deadline and 30 ms watchdog is allowed. The nominal due ticks below are the first ticks beyond equality. With only heavy's supervision disabled, it exited 23. The separately faulting app reported 37 in both boots.

| Exact published input, 62.5 MHz | Nominal due | Observed | Delay |
|---|---:|---:|---:|
| Heavy deadline, watchdog on | 1,473,294 | 1,477,827 | 4,533 ticks / 72.528 µs |
| Heavy watchdog, terminal status −110 | 2,410,794 | 2,415,213 | 4,419 ticks / 70.704 µs |
| Heavy deadline, watchdog off | 1,624,764 | 1,629,293 | 4,529 ticks / 72.464 µs |

After the stop, late exited 24, the sensor observer 22, the heartbeat 25 and the sensor producer 21. In the 20-app guest, all 20 tasks exited 25, each at private count eight. QMP page-table walks found 20 distinct backing pages for the same virtual address. The three confirmation policy tables, including embedded application binaries, match byte-for-byte the on, off and scale tables in main's published acceptance. All five original ELF identities and task declarations match those manifests.

감시를 켜면 heavy는 첫 릴리스 후 30 ms를 넘어 IRQ30에서 중단됐다. 나머지 정상 태스크 네 개는 이후 각각의 종료값을 남겼다. 감시를 끈 동일 실행 파일은 23으로 종료했다. 20개 앱은 같은 가상주소에서 서로 다른 물리 페이지에 최종값 8을 기록했다.

The first three policy boots used another WSL build of the Rust heartbeat. All loaded segments, permissions, addresses, sizes and bytes matched the approved heartbeat. Unloaded ELF data differed. Those boots and their exact inputs are retained under `results`. The separate three-position confirmation under `exact` used the archived whole-file ELF identity. All six policy boots used Cortex-A53 TCG, GICv2, one vCPU and 128 MiB.

## Fused permit and ordinary buffer loans / 결합 허가 연산·일반 버퍼 대여

The FIFO and RR packages used the unchanged sender/controller/supervisor ELFs, original wall-clock TCG, Cortex-A72, one vCPU, GICv2 and 128 MiB. The RR quantum was 625,000 ticks. Both kernels produced exits 71/72/73, zero private role errors and zero qualifier and safety journal losses. The general fixed journal dropped 278 suffix rows in each run. Each accepted eight commits, joined eight received payloads to sender bytes, recorded 64 successful PID1 SVC63 calls, and sampled virtual ENABLE=1 at 18 and 37 ms. All 32 completion timestamps joined their syscall intervals.

| Ticks at 62.5 MHz | Limit | FIFO healthy | RR healthy |
|---|---:|---:|---:|
| Maximum controller phase, lower–upper | ≤62,500 | 115,752–115,815 | 121,713–121,774 |
| Maximum job upper | ≤312,500 | 186,007 | 187,528 |
| Maximum sender upper | ≤625,000 | 206,400 | 215,236 |
| Maximum adjacent COMPLETE gap upper | ≤312,500 | 223,511 | 206,305 |
| Maximum grid PPI30 return upper | ≤31,250 | 26,503 | 27,780 |

Each policy had 32 closed controller intervals and complete pre-wake evidence. In FIFO and RR, job 0 phase A and job 1 phase A crossed the independent 62,500-tick limit. Job 0 phase B closed at 18,959–19,007 ticks in FIFO and 27,023–27,076 ticks in RR. All controller intervals finished before the next release at 156,250 ticks.

The independent PR candidate cohort at `d2bd3909` (same runtime sources as public `f30db111`) previously observed maxima of 98,708–98,758 ticks over four healthy FIFO boots and 128,211–128,268 ticks over four healthy RR boots. Those captures also recorded eight commits and ENABLE at both sample times. Both cohorts refuse the phase limit. These individual wall-clock measurements do not establish a speed change.

일반 FIFO 대여는 73/74로 종료했다. C 생산자와 Rust 소비자는 메시지 여덟 개를 처리했다. 가득 찬 큐에서 −11을 세 번, 재사용 주소의 이전 핸들에서 −116을 기록했다. 종료 시 head/tail 8/8, 점유 0, 빈 페이지 3장, 매핑 전이 34건 중 누락 0건이었다. 보호 채널의 두 정책은 각 위상 32개의 시작·완료를 전부 확인했으며 최초 두 작업의 A 위상이 제한을 넘었다. 실물 장치와 물리 WCET는 측정 범위 밖이다.

The ordinary FIFO loan guest returned 73/74, confirmed eight lengths and payload sums, three retries on a full ring, stale status −116, terminal head/tail 8/8, all three pages free, and 34 recorded mapping transitions with zero loss.

## Reproduction / 재현

Extract the packet into an empty directory. `SHA256SUMS` covers captures, complete kernel and application ELFs, package manifests, source/tree receipts, build and launch commands, raw UART, QMP RAM observations, references and the reducers. The original absolute paths are preserved in measured logs. GNU AArch64 binutils and Python 3 suffice for offline replay.

~~~sh
sha256sum -c SHA256SUMS
PYTHONDONTWRITEBYTECODE=1 python3 verify.py results exact > /tmp/jnox-integration-findings.json
cmp findings.json /tmp/jnox-integration-findings.json
~~~

For fresh boots, use Linux, Git, Python 3, Rust 1.95.0 with the `aarch64-unknown-none` target, AArch64 binutils and QEMU system AArch64. The recorded reducer pins the measured QEMU 6.2 binary identity. Keep the absolute output path short enough for an AF_UNIX socket. Run from the unpacked packet with fresh result paths. All runs are serial. New wall-clock measurements vary.

~~~sh
git clone --filter=blob:none --no-checkout https://github.com/Romanof123/JNOX.git src
git -C src sparse-checkout init --cone
git -C src sparse-checkout set arch boards kernel examples sdk services tools
git -C src checkout --detach 52e4a8b23897801a418da23cac18fefca4cea35b
python3 replay.py src /tmp/jnox-integration-first
python3 replay.py src /tmp/jnox-integration-exact --approved-policy
python3 verify.py /tmp/jnox-integration-first /tmp/jnox-integration-exact > /tmp/jnox-integration-new.json
~~~

[Raw integration captures, commands, checksums and replay](native-permit-policy-integration.tar.gz)

[Raw captures and offline replay](native-permit-policy-integration.tar.gz) (SHA-256 `de46a85144ccf12e959943c0f8b0c6558c922237a27641f4edf3e542d7c3460b`).
