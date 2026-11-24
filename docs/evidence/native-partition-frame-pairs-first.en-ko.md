# Native partition frame journal / 네이티브 파티션 프레임 기록

Parent f7d62aaea34ca4d298c5e052747f4f0e6602f76b, candidate 01521256ef1052d07a9fc73b82f7b38af0a70a00, plan c04c00236cefaa076e561f972bb0e256c47fd53d. Serial WSL ext4, CARGO_BUILD_JOBS=1, one QEMU Cortex-A72 per cold run. Five pairs per cell (first BC plus BC, CB, CB, BC). Bound 62,500 ticks at 62.5 MHz. Historical first P/default generation 2 was 73,392 ticks. Four historical originals are retained separately.

|Table / clock|Revision|First max|First miss generations|Boots over limit / 5|Windows over limit|Max across 5|
|---|---|---:|---|---:|---:|---:|
|P / default|base|87881|2|5|5|87881|
|P / default|cand|70513|2|5|5|95699|
|Q / default|base|67928|2|4|4|67928|
|Q / default|cand|63900|2|4|4|83150|
|P / icount3|base|1453|none|0|0|1453|
|P / icount3|cand|1432|none|0|0|1432|
|Q / icount3|base|1462|none|0|0|1462|
|Q / icount3|cand|1439|none|0|0|1439|

Every fresh P/Q boot completed the exact parent parser. It verified all 104 restored words against the private saved frame, stopped RAM and private address spaces, true EL0 returns and IRQ/SVC transitions against QEMU exceptions, readiness, explicit idle, positive private 3n/5n work and exits 41/42. frames.csv independently fingerprints both banks for every dispatch including drains and checks consecutive restoration per PID. stages.csv gives raw d, i, selection, r and x plus each interval for every timed generation. First generation uses the epoch for i. boots.csv and pairs.csv retain first and repeat values and paired deltas without pooling clocks.

두 버전의 기본 시계 최초 P/Q에서 모두 2세대 한도 초과가 발생했다. 결정적 시계 최초 P/Q는 모두 통과했다. 시간 초과와 무관하게 모든 P/Q 부팅에서 실제 EL0 실행, 전용 주소 공간, 104워드 문맥 복구, IRQ/SVC 및 두 작업의 정상 종료를 원판 재생기로 확인했다. 기본 시계에서는 수정본이 원래의 1 ms 기준을 보장한다는 결론을 내릴 수 없다. 가상 장치 관측만으로 지연의 원인을 특정할 수 없다.

The first default-clock P and Q pairs both missed generation 2 in both revisions. The candidate does not establish a 1 ms bound on the default virtual clock. The stage timings alone do not identify a causal speedup or host cause.

## Compatibility / 호환성

- compat-startup-probe-default-cand: parent replay exit 0
- compat-weighted-share-default-cand: parent replay exit 0
- compat-weighted-share-icount3-cand: parent replay exit 0
- compat-weighted-wake-default-cand: parent replay exit 0
- compat-weighted-wake-icount3-cand: parent replay exit 0

[Original forty captures and compatibility cases / 40회 콜드 부팅과 호환성 원본](native-partition-frame-pairs-first.zip) · SHA-256 c24226ba7fa7333f406d7ff8ab020e76a255ccb1a67ec1a412e68914d3880d6a.
