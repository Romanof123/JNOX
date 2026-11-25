# Weighted analytics on one QEMU CPU / QEMU 단일 CPU의 가중 분석 작업

[#304](https://github.com/Romanof123/JNOX/issues/304) · CHG-120 / IMP-120 / EVD-120 · [#21 POL-011](scheduling-release-cases.en.md). Prospective deployment profile on accepted main `33ee8c35a62ede54987f9af4ae51d3f2fcea3d35`. PM may select **weighted-analytics** as the named virtual application mix: independent C and Rust programs compute byte-stream integrity, distribution and energy, with a timed monitoring variant. Systems owns this profile's finite inputs and expected outputs; independent Verification scores the release candidate. The selection decision must identify the exact revision, machine, clock and variants.

PM은 가상 배포 대상으로 weighted-analytics를 선택할 수 있다. 이 프로필에서 C·Rust 앱은 바이트 스트림의 무결성, 분포, 에너지와 시계 관측값을 계산한다. 시스템팀은 유한 입력과 기대 결과를 관리하고 독립 검증팀은 선택한 릴리스 소스와 이미지에서 판정한다.

## Operator build and boot / 운영자 빌드·부팅

Provide one source-controlled operator command with these proposed normal entry points, run from the repository root on Linux/WSL:

~~~sh
python3 tools/weighted_analytics.py build --out output/weighted-analytics
python3 tools/weighted_analytics.py boot --out output/weighted-analytics --variant pair
~~~

Build all four ordinary ELFs once using the existing C SDK build.sh and Rust build-rust.sh and build the separate signed-distance error control. Use the same user binaries wherever a variant reuses a client. Call tools/build_native_el0.py with normal `--policy weighted`, ordered `--elf/--peer-elf`, corresponding weights and `--quantum-ticks 625000`. Preserve its kernel.elf, per-slot ELF copies, manifest and logs for each variant. Boot selects the exact named package with stock qemu-system-aarch64, virt/GICv2, Cortex-A72, TCG, one CPU, 128 MiB, serial console and default architectural clock. Require measured F=62,500,000 Hz for this profile. Store the exact argv and identities; reject an absent or mismatched package. A normal boot needs neither the verification archive nor the verifier's workstation paths.

Commit the extended data generators, loop extents, full product checks, a source-readable configuration and an independent host oracle. Adapt the existing weighted examples with explicit build-time input sizes or check in the small extended sources; retain the old examples' default extents and checks. Identify the historical extended sources and archive digest in the usage guide. Fresh builds must not unpack old captures, download binaries, vendor toolchains or copy a kernel backend. Keep growth in the logical dataset, generated a block at a time. Document Rust toolchain/target, clang/lld and QEMU prerequisites. Build into a new output directory; record source revision/changes, SDK and workload source/input hashes, compiler and linker versions/commands, per-slot ELF SHA-256, kernel digest, weights and ordered package metadata. Record actual produced identities, even when a toolchain produces different bytes. [Ordinary package](../tools/build_native_el0.py) · [Prior input and capture provenance](evidence/native-weighted-applications-first.en-ko.md)

저장소에 확장 데이터 생성 규칙과 반복 수, 독립 계산식을 넣는다. 기본 예제의 입력은 보존한다. 운영자는 빌드 명령 한 번으로 기존 SDK와 패키저를 통해 네 프로필과 오류 대조 이미지를 만들고 이름으로 부팅한다. 실행 파일은 변형 사이에서 재사용하고 각 이미지의 실제 해시·도구·빌드 인자·슬롯별 가중치를 기록한다.

## Frozen clients / 고정 입력과 결과

| Client | Extended workload, progress and final private product | Exit |
| --- | --- | ---: |
| C checksum | 524,288 ramps of bytes 0…255; 134,217,728 bytes. Reflected CRC-32/ISO-HDLC 2,124,873,618; independently reproduce via zlib. | 237 |
| Rust histogram | 2,097,152 blocks, byte (offset+block) modulo 256, popcount bins. 536,870,912 samples. Each of nine u32 bins equals 2,097,152 × binomial(8,k), k=0…8. | 70 |
| C energy | 4,194,304 ramps −128…127; 1,073,741,824 samples. Unsigned u64 energy 5,864,240,971,776 equals block count times the sum of squares. | 179 |
| C timed reporter | Three ordinary absolute monotonic waits of 40 ms each; retain each requested and observed timestamp. Final progress 3, success mask 7. | 7 |

The first three datasets are exactly eight times the original #297 extents. Private RAM, disassembly, complete counters/products and each exit must agree. Previous observations provide traceability, not expected identical physical timing or fixed addresses across toolchains. [EVD-116](evidence/evd-116.en.md)

## Original limits on this deployment / 이 배포의 판정 기준

Freeze the checkout, builds, tool versions, oracle, cold positions, QEMU argv, RAM journal collection, timeout and capture storage before the first boot. Use first default-clock positions in order **pair, swap, triple, timed**, followed by the dedicated overflow control. Q=F/100=625,000 physical ticks (10 ms), H=5Q/4=781,250 and G=Q/4=156,250. Reconstruct rational virtual times, minimum selection, monotone floor, lease identity, service, membership and compare from independent raw stamps. Exclude scheduler gaps and idle from charged service. In contended windows a full timer charge is in [Q,H], intervening dispatch gaps ≤G; apply the original upper bound H to charged contended intervals. Show private user computation and actual save/restore via the packaged ELF symbols and address spaces. Keep post-window tails and blocked/solo intervals distinct. Physical service charged to an execution interval includes the original measurement brackets; it is not an instruction-only count.

| Package | Slots (ELF:weight) | Frozen case |
| --- | --- | --- |
| pair | checksum:1, histogram:2 | First 36 full grants, both continuously ready. At each prefix, for services A of the weight-1 app and B of the weight-2 app, −H ≤ 2A−B ≤ 2H. At 36, T=A+B ≥ 36Q and abs(2A−B)·36Q ≤ 2H·T. |
| swap | checksum:2, histogram:1 | Same ELF order, reversed weights. Repeat all 36-grant and prefix checks with A and B defined by weight. |
| triple | checksum:1, histogram:2, energy:3 | First 60 contiguous full contended grants, all three ready. At every prefix from zero, max(Sᵢ/wᵢ)−min(Sᵢ/wᵢ) ≤ H/min(w), exact rational comparison. Verify minimum selection, progress of all three and their independent final results. |
| timed | checksum:1, timed reporter:2 | For each of three actual 40 ms absolute sleeps, checksum progresses and accrues ≥3Q while reporter blocked service stays zero. Verify placement on wake, retained state, first returned frame and reporter clock reads. |
| overflow | independent C SDK error probe plus a productive peer with weights 1:2 | Confirm the signed-distance rejection on the actual candidate and the continued service of both applications, as specified below. |

For the timed case measure D (target), I (IRQ), W (wake accepted), R (first actual user return), [L,U] (post-wake user clock sample). Apply D≤I≤W≤R≤L≤U, I−D≤G, W−I≤G, R−W≤H+2G, U−R≤G and U−D≤H+5G for each wait. Evaluate uncertain endpoints conservatively using the restore hook as a lower bound and a subsequent observed user clock trap as an upper bound on return. Keep an inconclusive bracket partial. A deliberately uninterrupted solo lease to a wake is outside the contended full-grant upper bound. Retain all three recorded private deadlines and post-return clock values. [Original numerical contract](native-weighted-applications.en-ko.md)

오리지널과 동일하게 10 ms 실행권으로 판정한다. pair와 swap은 실제 경쟁 36회, triple은 60회의 모든 접두 구간에서 표의 산술식을 검사한다. timed는 40 ms 절대 대기 세 번 모두에서 동료의 실제 서비스가 3Q 이상이고 대기자의 정산이 0이어야 한다. IRQ·깨움·사용자 복귀·복귀 후 시계 표본은 원래 범위를 보수적으로 판정한다. 과거의 특정 스위치 횟수나 측정 시간을 새 합격선으로 쓰지 않는다.

## Signed-distance control and release decision / 거리 오버플로와 승인

At F=62.5 MHz, submit an absolute CLOCK_MONOTONIC request for 160,000,000,000 seconds, nanoseconds zero, through native clock_nanosleep(TIMER_ABSTIME). Capture the actual epoch/counter and independently show that epoch + 10,000,000,000,000,000,000 fits u64 while target−now > i64::MAX. This reaches the signed-distance guard, distinct from malformed timespec or u64 conversion overflow. Require SDK return EOVERFLOW (75), immediate continuation and a private success result. Correlate the call PC and return with the journal: no sleep/block/wake generated for that request, same lease and armed compare through the nonblocking call, and no release, renewal or charge caused by it. A real timer before or after the syscall remains separately accountable. Follow with a successful representable wait, a resumed user computation and both expected exits. Freeze the finite peer and its own result in advance; distinguish measured scheduler decisions from raw timestamps that naturally advance. Keep this error probe outside the four historical data selections. [Signed-distance handling](../kernel/src/native_el0.rs)

현재 계측 주파수에서 1,600억 초 요청은 u64에는 들어가지만 현재 시각과의 차이가 i64::MAX를 넘는다. 실제 에포크로 이를 다시 계산한다. EOVERFLOW=75, 즉시 복귀, 요청에 의한 차단·실행권·타이머 변경 없음, 후속 정상 대기의 복귀와 두 앱의 종료를 각각 확인한다.

Publish whole first journals and drop counts, actual boot identities, RAM outputs, independent reconstruction, launch failures and any missing window. Verify ordinary command → package → boot on the release tree. Record timing, policy correctness, functional outputs and EOVERFLOW as separate outcomes. The original short runs (25/36, 13/36, 49/60), truncated initial icount run, #177/#178 and open #184 retain their original dispositions. Four extended default runs on `100d32e` passed the weighted windows. They did not exercise this error call. A successful new cohort supplies the #21 POL-011 application-mix observation for this named QEMU configuration if PM adopts it. PM separately records any family release decision. A larger product workload, physical timing or safety approval requires its own configuration and authority.

최초 원본과 누락 여부, 실행한 ELF 및 커널 해시, 사설 메모리 결과, 산술 재생 근거를 보존한다. 새 결과는 PM이 선택한 단일 QEMU 배포의 가중 서비스·깨움 판정에만 귀속된다. #184와 기존 최초 실행, 전체 제품 부하 및 실물 보드 승인은 각각 별도다.
