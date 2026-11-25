# Native permit timing screen / 네이티브 허가 채널 타이밍 판정

Use the extracted FIFO or RR qualification capture with its original pkg/manifest.json and cap/launch.json, kernel.elf, session.json and uart.bin. The package kernel and captured user.elf are required. The checker hashes each entire embedded ELF in the kernel and matches the ordered UART identities. It also hashes every staged slot ELF present in pkg/ or cap/ and requires each ELF listed in the capture receipt. The first-position archives retain their two peer ELFs inside the kernel. For the original first-position archive:

~~~sh
mkdir fifo-healthy
tar xf fifo-healthy.tgz -C fifo-healthy
python3 tools/native_safety_admission.py fifo-healthy > fifo-healthy-admission.json
~~~

The supported deployment comprises the 24 frozen #389 first-position package configurations. Their digests cover every manifest field after normalizing the three source locations and the output location. Other configurations need separate qualification. The launch receipt must identify the frozen QEMU 6.2.0 executable SHA-256, virt/GICv2, Cortex-A72, one CPU, TCG, 128 MiB, default clock, original device options and associated kernel/UART paths. The capture records the emulator digest. Its executable is external. RAM extent and receipt, probe and 62.5 MHz counter are checked. Reordering flags or adding clock/device switches gives an unsupported-input result.

원본 24개 패키지 설정과 QEMU 실행 인자 전체를 대조한다. 호스트 소스와 출력 경로는 이동할 수 있다. CPU·타이머·메모리·장치 또는 패키지 옵션이 다르면 구간을 계산하기 전에 보류한다. 에뮬레이터 해시는 수집기에 기록된 값이며 실행 파일은 패킷 외부에 있다.

The JSON preserves policy, image hashes, frequency and journal row indices. Exit 1 means **FAIL** with an observed contradiction. Exit 2 means **UNKNOWN/HOLD**, including incomplete or malformed evidence. A successful guest exit alone cannot admit this workload. The entrypoint consumes timing rows directly and leaves the protected-frame, physical-memory and safety-event decoding to the independent qualification replay.

The check requires distinct priorities in the order controller, sender, supervisor. It checks observed dispatch against each recorded new runnable mask, 16 pairs of controller targets 2.5 ms apart and eight sender targets 10 ms apart. A/B belong to the same controller. Each segment runs through the actual scheduler block at the next stage-1 invocation or final park. The stage-3 vector sample is a lower bound; the scheduler's before/after selection samples tighten that bound and supply an upper endpoint when the running slot leaves the runnable mask. The upper endpoint includes bookkeeping through the block, excluding subsequent work of other slots. All comparisons use integer counter ticks without rounding at inclusive limits. A lower endpoint above 1 ms per controller phase or 10 ms per sender job refuses the observed configuration. An upper endpoint crossing a limit with its lower endpoint inside is unresolved. The inclusive 1 ms ceiling applies to the qualification profile's phase response. A failure after an earlier decision cannot by itself establish that the earlier check was late.

**Separate numerical evidence.** An optional second argument `--bounds FILE` checks independently justified end-to-end response envelopes against each segment. The JSON object contains the exact `kernel_sha256`, `policy`, and `bounds`. Its keys A, B and sender each take an integer `response_us`, a document `artifact` path relative to FILE, its `sha256`, and a nonempty `section` identifying the justification. Missing entries remain unresolved. The number must bound nominal target through whole-segment block, accounting for jitter, all CPU demand and interference, both timing endpoints, and the same executable and observation configuration. This interface does not interpret EL0-only WCET as a response limit. Artifact integrity verifies which analysis was supplied. Qualification of its assumptions and acceptance of its numbers require independent review.

**Why admission stays open.** SVC 62 returns immediately at already-due targets. Dynamic compare programming, the supervisor's off-grid 19.9 ms lower-work activation, RR timer events, nonpreemptible work, carry-in pending IRQs and instrumentation all require a model. The frozen FS-M3 recurrence assumes a periodic timer on a 2.5 ms grid. These observations cannot prove that model for all jobs. The tool retains the native FIFO/RR policy throughout. A clean finite timing screen still needs an applicable proof and justified worst-case service, release jitter, blocking, IRQ and two-endpoint uncertainty bounds. Physical output, FTTI, hardware interrupt assertion latency, and product ASIL are outside the virtual CPU timing screen.

## 한국어

압축을 푼 실제 게스트 디렉터리를 첫 번째 인자로 전달한다. 종료 코드 1은 실측 하한 초과·실행 순서 위반 등 확인된 실패, 2는 근거 부족으로 보류함을 뜻한다. JSON에는 원본 해시, 정책, 주파수, 작업별 목표·구간·차단 행이 보존된다. 제어·송신·감독 순위와 실제 선택, 16개 A/B 쌍과 8개 송신 작업의 절대 격자를 확인한다. 구간 종료에는 사용자 작업 이후 다음 실제 차단까지의 기록·커널 비용을 포함한다. 1 ms 및 10 ms 경계는 정수 틱으로 비교한다. 구간 전체의 초과와 안전 판단 자체의 지연은 별도로 평가한다.

선택적 수치 입력에는 해당 커널·정책의 구간 응답시간 상한과 외부 근거 문서의 해시·절을 지정한다. 문서 해시 확인만으로 최악 실행시간이나 IRQ 간섭 상한이 증명되지는 않는다. 현재 구현의 절대 대기는 목표를 지나면 즉시 복귀하며 감독자의 19.9 ms 동작은 2.5 ms 격자 밖이다. FIFO/RR를 원래 정책으로 유지하고 FS-M3-01의 RM/DM 분석을 호출하지 않는다. 일회 실행 관측으로 모든 작업의 스케줄 가능성이나 물리 출력 안전 등급을 승인할 수 없다.

## Later reviewed deployments / 후속 검토 배포

For the corrected candidate, use the extracted case, the reviewed profile file, and the SHA-256 of its exact bytes from an independent release record.

~~~sh
python3 tools/native_safety_admission.py "$CASE" \
  --deployment-profile "$REVIEWED_PROFILE" \
  --deployment-profile-sha256 "$INDEPENDENTLY_RECORDED_PROFILE_SHA256"
~~~

[Example profile for the original FIFO healthy case](../tools/native_safety_deployment_example.json). Each position key is POLICY-CASE with an additional -irq for a manifest declaring safety_interference. Each entry approves one policy, clean commit, canonical manifest digest, kernel digest, ordered sender/controller/supervisor hashes and RAM extent. Review the candidate source, package command, linked binary, emulator and memory map before freezing these values. The platform must match the documented virt GICv2 Cortex-A72 single-core TCG deployment. A reviewer may pin another emulator build on the same platform. The exact launch arguments, capture receipt, embedded/staged ELF hashes, kernel linkage and counter frequency checks still apply. There is no wildcard position or fallback to built-in approvals after an external-profile rejection.

Compute the canonical manifest hash from an independently reviewed build at the repository root:

~~~sh
python3 -c 'import json,sys; sys.path.insert(0,"tools"); from native_safety_admission import deployment_digest; print(deployment_digest(json.load(open(sys.argv[1]))))' "$REVIEWED_PACKAGE/manifest.json"
~~~

Only three slot source locations and the package output location are normalized. Freeze the profile file and record its exact byte hash through the trusted release channel. A digest drawn solely from the capture being assessed supplies no independent authorization. Release review establishes that the declared settings correspond to the approved binaries. Deployment identity and numerical timing-bound justification have separate inputs.

후속 후보에서는 독립 릴리스 기록에 고정한 프로필 SHA-256과 JSON 파일을 함께 전달한다. 각 항목은 정책·사례, 커밋, 정규화한 매니페스트 해시, 커널·순서가 지정된 ELF 세 개 및 캡처 범위를 담는다. 프로필을 제출해도 Cortex-A53, 다른 타이밍 모드, 추가 장치 또는 ELF 변조는 허용되지 않는다. 패킷 안의 자체 해시만으로 승인하지 않는다.
