# VER-019 · Closed producer gate and downstream permit traffic / 상류 게이트 차단 후 하류 허가 입력

The first cold boot of each of six ordinary AArch64 QEMU virt cases satisfied the observed 25 ms detection, 5 ms inhibition and 30 ms total response limits. Both policies inhibited the virtual output while the closed producer gate continued to refuse normal acquisitions. Every measured workload boot, including both healthy controls, exceeded its separate 1 ms controller phase budget twice.

FIFO와 RR의 무송신·손상·포화 첫 부팅 여섯 건에서 고장 검출 25 ms, 검출 후 출력 차단 5 ms, 전체 30 ms 이내를 관측했다. 게이트를 닫은 뒤 정상 송신기의 획득이 거부되는 동안에도 감독자가 넣은 손상 프레임을 실제 수신·반환했고 가상 출력이 내려갔다. 정상 대조군을 포함한 시각 측정 부팅 열 건은 모두 제어기 위상 두 개에서 독립적인 1 ms 한도를 초과했다.

## First captures / 최초 원자료

The package source is `b9c4d7aafed05aeecc695dc39f0c81f4e3a7367e`, with tree `b57eb56d86185b984b3117c7af121ba76b1d8ac5` and parent `a7aa0a659f239950fa29b3326813cf4b35d846f9`. The archive carries the commit and root tree bytes, audited build-source subtrees and all 12 original boots. The package manifests record board, scheduler, binary digests and role arguments. Each campaign package used the standard packager, protected channel, qualification recorder, three processes at priorities 6/7/4 and two-entry, three-page 0→1 loan route. RR used 625,000 ticks. Gate-invalid and gate-saturation packages enabled the explicitly named downstream fixture. Both no-option controls used the gate-invalid arguments without the selector. The two separate guard packages used the same opt-in and externally supplied C test roles.

All clocks below are 62,500,000 Hz on single-core Cortex-A72 TCG with stock QEMU virt/GICv2. Qualification recorded actual CNTP_CVAL_EL0 readbacks of programmed targets and physical timer INTID 30. The launch records retain emulator hash, arguments and version. The normal controller used separate CHECK and COMPLETE services. The first two valid issues were scheduled exactly 10 ms apart for rearm. The third issue was a live creation tick.

실행 소스와 Git 객체, 빌드에 사용한 하위 트리, 매니페스트, 역할별 ELF 및 UART/RAM 원본을 보관했다. 가상 물리 타이머 비교값과 PPI 30을 기록했다. 재무장을 위한 첫 정상 프레임 두 개의 발행 기준은 합성 일정이고, 세 번째 프레임은 실제 생성 시각을 담았다.

## Virtual output and timing / 가상 출력과 시각

The fault-injection bracket [C0,C1] is the supervisor's event 4 around CLOSE and grant invalidation. For detection [D0,D1], D0 is the real counter operand for expiry and D1 is event 3's journal timestamp after the protected fault latch changed. The first 1→0 virtual-register store and readback form [S0,S1] in event 2. All six captures have C1 < D0 ≤ D1 < S0 ≤ S1 and a later endpoint exit. The three independent conservative limits below use D1−C0, S1−D0 and S1−C0. CHECK gaps join the 32 expiry decisions by their actual counter operand. COMPLETE gaps use the later service's upper endpoint minus the earlier service's lower endpoint for each of the 31 consecutive pairs, including B→next A.

표의 값은 ms이다. CLOSE, 보호 고장 래치, 가상 출력 1→0의 각 관측 구간은 순서가 명확하다. 고장 주입 하한에서 검출 상한, 검출 하한에서 안전 출력 상한, 고장 주입 하한에서 안전 출력 상한을 각각 뺐다. CHECK 시각은 실제 판정 입력이며 COMPLETE 간격에는 양쪽 서비스 시각의 불확실성을 포함한다.

| Policy / 정책 | Original input / 입력 | CLOSE→detect max | Detect→safe max | CLOSE→safe max | Max CHECK gap | Max COMPLETE gap | Fixture enqueues / full |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| FIFO | silence | 19.922176 | 0.029136 | 19.926784 | 2.738992 | 3.490112 | 0 / 0 |
| FIFO | gate-invalid | 19.680240 | 0.028320 | 19.684896 | 2.684480 | 2.740416 | 7 / 0 |
| FIFO | gate-saturation | 19.610384 | 0.027328 | 19.614800 | 2.599360 | 3.523664 | 14 / 7 |
| RR | silence | 19.637728 | 0.027632 | 19.642080 | 2.559280 | 3.326656 | 0 / 0 |
| RR | gate-invalid | 19.644128 | 0.030880 | 19.649328 | 2.591424 | 3.450976 | 7 / 0 |
| RR | gate-saturation | 19.610352 | 0.022176 | 19.613984 | 2.569520 | 2.800688 | 14 / 7 |

## What the traffic established / 실제 대여와 출력

* All six target boots showed ENABLE high at E+18 ms and E+37 ms and the valid sequence 3 immediately before CLOSE. Last valid sequence, issue and private recovery count persisted through expiry. The protected sampler latched the fault after the last valid issue aged to 20 ms. All five subsequent normal upstream acquisitions returned EPIPE in each case. The sender, controller and supervisor exited 71/72/73.
* Each invalid boot injected seven unique loan handles. Each saturation boot accepted 14 out of 21 requests. All seven refusals showed occupancy 2→2 and EAGAIN, with a later real dequeue and successful publication. Every successful handle matched the protected 44-byte receive snapshot, the read-only slot-1 mapping/revocation and the controller's release. Before expiry, invalid-only and saturation runs had three and six CRC rejections respectively. The four and eight remaining frames returned the already-faulted private receiver status. None committed. At least one successful input occurred on either side of expiry. Each targeted boot had exactly the three pre-CLOSE protected commits.
* The fixture was `01012c000100010101000000000000000400000000000000000000000000000014000000010000000995750b`, with stored little-endian CRC 0x0b759509; independent CRC-32 was 0x0b759508. In each standard workload boot, the last received frame in every used physical loan page matched its 44 bytes in terminal RAM and all 4,052 tail bytes were zero. Reused slots only retain their latest historical frame. The dedicated per-policy guard captured both full pages while slot 1 held each live read grant after the upstream sender had filled all three old pages with 0xA5. Both copies contained this 44-byte fixture and a completely zero tail.
* Each healthy boot accepted sequences 1–8 and withdrew on 8. In both no-option boots all seven supervisor requests returned EINVAL, event 14 was absent, and the supervisor exited 96. The capture tool's marker is slot 0 exit 71 in these controls. The decoder checks all three exits to identify the intentional negative outcome.

정상 허가 순번 3과 시각, 전용 수신기의 복구 카운트가 유지됐고 차단 뒤 송신 획득 다섯 번은 매번 EPIPE였다. 포화 사례의 일곱 번 거부는 모두 실제 큐 점유율 2→2였고, 이후 큐가 빠진 뒤 새 핸들이 성공했다. 수신 핸들·바이트·반환·오류는 독립 자료로 연결했다. 무옵션에서는 일곱 차례의 명령 5가 모두 EINVAL이었다. 전용 대조군은 먼저 전체 페이지를 0xA5로 채운 뒤 새로 대여한 페이지의 44바이트와 나머지 4,052바이트 전부를 직접 복사했다.

## Kernel guards and false claims / 커널 경계와 반증 대조군

FIFO and RR each ran an additional boot exercising role, lifetime, gate, generation and quota checks. Wrong slots 0 and 1 returned EACCES; an open gate returned EBUSY; generation zero, a wrong generation and a stale generation returned ESTALE. At CLOSE the three old pages were in reader-held, queued and writer-held states. The kernel revoked both mapped leaves and discarded the queued handle. The old reader release returned ESTALE after the test path became available. Normal writer publication/release returned EPIPE while closed and the old handle returned ESTALE after a new OPEN. All leaves ended invalid.

Generation 1 accepted its first two fixture requests, charged the next 62 real full-queue refusals, and returned EOVERFLOW on attempt 65 with occupancy 0→0 after both previous fixtures were drained. Generation 2's independent quota admitted a new successful handle after CLOSE. A further request returned EPIPE after slot 1 exited. The fixture never changed the virtual output in these guard-only boots.

The package suite rejected nine malformed CLI combinations. The compiled, unmodified kernel build script rejected eleven direct configurations and accepted matching ordinary and fused controller variants. The no-selector positive emitted no downstream configuration flag. Six checked-in host loan tests passed. The C and Rust wrappers both use SVC 59, command 5 and generation in x1, observed separately through the guard C supervisor and standard Rust supervisor in the guest. Seven detached replay mutations were refused, including torn occupancy, a stale handle, an altered receive CRC, a late safe endpoint, private recovery advancement, nonzero tail and a falsely successful 65th attempt.

두 정책 모두 실제 QEMU에서 잘못된 역할, 닫기 전 호출, 잘못된 세대, 종료 뒤 호출을 거부했다. 기존 쓰기·읽기 매핑을 철회하고 대기 프레임을 폐기했다. 성공 두 건과 큐 포화 거부 62건 뒤, 큐를 모두 비워도 65번째 명령은 EOVERFLOW였다. 새 세대에서 다시 성공했다. CLI 거부 아홉 건, 직접 빌드 구성 거부 열한 건, 호스트 대여 테스트 여섯 건과 허위 근거 변형 일곱 건을 각각 확인했다.

## Independent timing shortfall / 독립 시각 한도 초과

Phase endpoints extend from nominal release through application logging to the effective blocking scheduler transition or final park. The retained rows bracket every controller endpoint. Each of the ten ordinary application boots had two definite misses and no uncertain 1 ms crossings. `A0` is the first phase, `B0` the second and `A1` the third. The six target boots meet their measured fault-response limits. General 1 ms phase admission remains open.

| Policy | Input | Definite misses | Worst phase upper (ms) | Worst sender upper (ms) | Generic fixed rows dropped |
| --- | --- | --- | ---: | ---: | ---: |
| FIFO | silence | A0, A1 | 1.928288 | 3.475648 | 323 |
| FIFO | gate-invalid | A0, B0 | 1.779664 | 4.091808 | 383 |
| FIFO | gate-saturation | A0, A1 | 1.947264 | 3.365280 | 439 |
| FIFO | healthy | A0, A1 | 2.043600 | 3.494256 | 342 |
| FIFO | denial | A0, A1 | 2.017152 | 3.535952 | 369 |
| RR | silence | A0, A1 | 2.112880 | 3.568912 | 323 |
| RR | gate-invalid | A0, A1 | 1.947712 | 3.427184 | 383 |
| RR | gate-saturation | A0, B0 | 1.428176 | 2.503664 | 438 |
| RR | healthy | A0, A1 | 2.356752 | 3.881984 | 342 |
| RR | denial | A0, B0 | 1.828688 | 4.272464 | 369 |

The safety journal, qualification recorder and loan mapping journal reported zero drops in every boot. The ordinary fixed scheduler trace retained a bounded prefix. All quoted endpoints and timer readings use the complete qualification records. These captures measure this single-vCPU virtual configuration. An independently bounded WCRT, loaded-target response and physical actuator timing remain open.

일반 스케줄러 로그는 표와 같이 일부가 누락됐다. 출력·판정·위상·매핑에 사용한 보호 기록, 검증 기록, 대여 매핑 기록은 모두 손실 0이었다. 1 ms 제어기 위상 수용, 최악응답시간의 독립 상한, 실물 구동기 동작 시간은 추가 검증이 필요하다.

## Offline replay / 오프라인 재현

Extract the archive into an empty directory and run `bash scripts/verify.sh .`. Linux, Python 3.10+, GNU AArch64 nm, sha256sum and rustc with Edition 2024 support are required. The detached verifier hashes the archived inputs, walks the process page tables, reproduces results, reruns the host package controls and compares the derived files. It does not start QEMU, fetch source or depend on the original filesystem paths in retained manifests. The original paths and emulator argv are provenance.

빈 디렉터리에 압축을 풀고 위 명령으로 모든 해시, Git 소스, 원본 UART/RAM·ELF 연결, 시간 구간, 빌드 거부 및 허위 판독 대조군을 다시 확인할 수 있다. `scripts/campaign.sh`와 `scripts/probe.sh`는 최초 실행 순서와 빌드·부팅 명령을 보존한다.

[Raw captures and offline replay](native-permit-closed-gate.tar.xz) (SHA-256 `eb4df7b2d1e1f6c064a18c2cd77533f3fe5463b4ecd2d9a1effc6b01677ca8e5`).
