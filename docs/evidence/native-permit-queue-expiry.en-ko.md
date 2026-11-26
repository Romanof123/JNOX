# Queued permit at expiry / 큐에 대기한 허가의 만료

A fresh frame in the queue can replace the previous grant while that grant is valid. Once the controller checks an expired grant, it latches the fault and turns off the virtual output before consuming the fresh frame. The six component boots tested that boundary to one tick. Twelve separate three-process boots measured when the kernel actually performed the check. Trace links run from REQ-019/024 through IMP-311 and VER-019/024 to EVD-311. [Implementation and verification #403](https://github.com/Romanof123/JNOX/issues/403) · [Open system acceptance #401](https://github.com/Romanof123/JNOX/issues/401)

기존 허가가 유효할 때는 큐의 새 프레임으로 허가를 갱신할 수 있다. 기존 허가가 만료된 것을 제어기가 확인하면 새 프레임을 꺼내기 전에 고장을 래치하고 가상 출력을 끈다. 구성요소 부팅 6회에서 한 틱 차이의 동작을 시험했고 세 프로세스 부팅 12회에서는 커널이 실제로 판정한 시각을 측정했다.

## Service behavior / 커널 서비스 판정

D is the accepted sequence-3 issue time plus its 20 ms validity. At 62.5 MHz one tick is 16 ns. Each independent AArch64 component boot established generation 1 with the output high and put the same CRC-valid 44-byte sequence-4 frame in the real loan ring. The test process links the kernel’s Rust safety service, loan service and ring. The fixture supplies role IDs, page-map operations and the CHECK time. The verifier read the register, ring records, service journal and frame bytes from captured RAM.

| CHECK time / 판정 시각 | FIFO and RR, one boot each / 각 1회 | Accepted sequence / 수락 순번 |
| --- | --- | --- |
| D−1 tick | Output stayed high. Receive, release and COMMIT succeeded. / 출력 유지, 수신·반환·갱신 성공 | 4 |
| D | CHECK latched expiry and set output low before dequeue. Receive and release succeeded, COMMIT returned −11. / 인출 전 차단, 수신·반환 성공, 갱신 거부 | 3 |
| D+1 tick | Same latch, output, queue and COMMIT results. / 정각과 동일하게 차단하고 갱신 거부 | 3 |

At and after D the previous issue time, generation and recovery count stayed intact. All six model outputs and original guest journals matched independently. The verifier rejected 150 modifications to copies of the results and RAM. The test drives the service clock explicitly. [Frozen AArch64 fixture and build instructions](https://github.com/Romanof123/JNOX/tree/4ae1c136988b8cd811f472267bfad3b4eb27d2f3/examples/native-permit-queue-contract)

D는 마지막으로 수락한 순번 3의 발행 시각에서 20 ms 뒤이며 카운터 한 틱은 16 ns다. 시험 앱은 실제 Rust 안전 서비스, 대여 서비스와 큐 코드를 링크하고 역할·페이지 매핑·검사 시각을 공급한다. 정각과 한 틱 뒤 모두 이전 허가의 발행 시각, 세대 및 복구 횟수가 유지됐다. 원본 RAM과 저널을 독립 판독했고 복제 기록의 변이 150건을 모두 검출했다.

## Three-process kernel observations / 세 프로세스 커널 관측

The producer, controller and supervisor ran on a single QEMU Cortex-A72 vCPU with the gate open. Each calendar ran under FIFO and RR with both the ordinary and instruction-count clocks. The new kernel observation reads occupancy, front handle and length inside the opening CHECK without consuming a grant. It records the same counter value C used by that decision. Queue residency at D follows independently from the publication bracket and the earliest possible dequeue. The stock calendars and communication path stayed fixed.

| Publication target / 게시 목표 | Boots / 부팅 | Sequence 4 queued at D / 만료 정각에 대기 | Queue read directly at expiry CHECK C / 실제 검사 시 큐 |
| --- | ---: | --- | --- |
| D−1 ms | 4 | Yes / 대기 | Handle 16, 44 bytes / 44바이트 |
| D | 4 | No, publication finished after D / 정각 이후 게시 | Handle 16, 44 bytes / 44바이트 |
| D+1 ms | 4 | No / 미게시 | Empty / 비어 있음 |

All 12 CHECKs ran **135–17,521 ticks (2.160–280.336 µs) after D**. Each latched expiry and inhibited the virtual output. The receiver later declined sequence 4 locally and skipped COMMIT. Terminal output was low in all 12. Direct queue observations matched replay of loan operations at **204/204** successful opening checks. All **2,004** safety rows matched between UART and captured kernel RAM. The safety and mapping journals were complete. The general scheduler trace dropped rows. Ten modified-record controls were rejected.

게이트가 열린 상태로 기존 세 일정을 그대로 실행했다. D에 정상 프레임이 대기한 부팅은 4회였고, 실제 만료 검사 C에서 큐 선두를 직접 확인한 부팅은 8회였다. 12회 모두 C는 D보다 2.160~280.336 µs 늦었다. 이 검사에서 출력을 차단했고, 이후 수신기는 순번 4를 거절해 COMMIT을 호출하지 않았다. 커널의 직접 큐 관측 204건과 독립 복원이 일치했다. UART와 커널 RAM의 안전 기록도 2,004행 전부 일치했다. 일반 스케줄러 추적 일부가 유실됐으며 판정에 사용한 안전·매핑 기록은 모두 보존됐다.

## Scope and open criteria / 검증 범위와 남은 조건

The older 12 captures from #401 were also re-read, independently of the 18 new boots. They gave the same four-at-D and eight-at-C counts. The 12 new three-process boots each had at least one completed-check interval above 5 ms. The evidence therefore leaves the following criteria open.

- An actual three-process CHECK with a queued grant and C exactly equal to D. This work also leaves coincident publication and CHECK on other execution platforms open. [#401](https://github.com/Romanof123/JNOX/issues/401)
- The full cadence and phase criteria, including the deferred 1 ms work. [#389](https://github.com/Romanof123/JNOX/issues/389)
- Hardware timing and output validation, functional-safety acceptance and release approval. IPC-010 remains partial. Nine of ten IPC families have virtual acceptance. [IPC tracker #24](https://github.com/Romanof123/JNOX/issues/24)

과거 원본 12회는 신규 부팅 18회와 별도로 재판독했다. D에서 4회, C에서 8회 대기한 결과가 일치했다. 신규 세 프로세스 부팅은 매번 적어도 한 구간에서 검사 완료 간격이 5 ms를 넘었다. 세 프로세스 커널의 정확한 D 판정, 다른 플랫폼의 동시 게시·검사, 전체 주기·위상 조건, 실보드 및 기능안전 승인은 열려 있다. 1 ms 위상 작업은 보류 상태다.

## Evidence and replay / 원본과 재현

The build commit `a69bc6948c1d57c6df89d395e9307de5e0c6d6dc` and [public implementation](https://github.com/Romanof123/JNOX/commit/4ae1c136988b8cd811f472267bfad3b4eb27d2f3) have the same Git tree `a851e772eff77659b16d9a372de6d353be803757`. Six host preflight attempts stopped before guest launch because the capture script expected a Boolean for an unset manifest field. The corrected preflight accepted the same images. The packets preserve the first six component and first twelve three-process guest launches.

로컬 빌드와 공개 구현의 Git 트리가 일치한다. 캡처 스크립트가 미설정 필드의 자료형을 잘못 예상하여 호스트 사전 점검 6건이 게스트 시작 전에 중단됐다. 동일 이미지로 점검을 바로잡았고 실제 최초 부팅 18회의 기록을 보존했다.

| Packet / 자료 | SHA-256 |
| --- | --- |
| [Six component boots, sources and verifier / 구성요소 6회](native-permit-queue-component.tar.xz) | `202400d143760747ac87d1a43da8f250108dd50950d61ab0ae022a6456a7e5fe` |
| [Twelve three-process boots, sources and verifier / 세 프로세스 12회](native-permit-opening-queue.tar.xz) | `6c55e5e85cb37a372929cbc8c4694f3c1a6f7b683a69fdb1cb8e3a893bdacdb7` |
| [Configuration matrix and seven loan tests / 설정 대조 및 단위 시험](native-permit-queue-observer-controls.tar.xz) | `c65f376de47a6aa5fa72bad38db197123f9d0e9136be6b28ef684d585cd6c87c` |
| [Historical independent replay / 이전 원본 재판독](native-permit-queue-residency-history.tar.xz) using [the original #401 captures](https://github.com/Romanof123/JNOX/blob/6727603b1df94acc709889ec708589099331c02f/docs/evidence/native-permit-arrival-first.tar.xz) | `041158939873ad359755fdff2b1b260da350f1c7b6dd3ffc9d7230244a9cff02` |

Extract each new boot packet in a separate directory and run `bash scripts/replay.sh .` from its root. The included checkers verify captured identities and raw observations. The configuration packet records agreement on 102 selections and the linked-kernel symbol checks. Only the six selected three-role arrival configurations enabled observation.

신규 부팅 패킷 두 개를 각기 다른 디렉터리에 풀고 각각의 루트에서 `bash scripts/replay.sh .`를 실행한다. 설정 대조는 선택 조합 102건을 검사했으며, 지정한 세 프로세스 도착 시험 구성 여섯 개에서만 관측 기능이 켜졌다.
