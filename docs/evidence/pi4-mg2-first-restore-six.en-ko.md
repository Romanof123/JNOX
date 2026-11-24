# Pi 4 first peer return / Pi 4 피어 최초 복귀
Plan 81eb09b2e2e6ca33f1c4cca50b44148766f5ab56. Release cc864a392828d37cf2fb933566e93e20bfeaa455. Stock raspi4b QEMU SHA-256 3ae2c079d4491362fece2abb641f0ff17d88e9008c9fcd13600f249b9bd1c862. Two active guest CPUs on four emulated CPUs. WSL ext4 sequential builds with CARGO_BUILD_JOBS=1 and sequential cold launches. Global 1/2, partitioned 1/2, clustered 1/2 in that order. First six preserved.

**0/6 qualified.** Every CPU0 trace shows ERET to M followed by M's ending SVC. CPU1 independently returned to P1, entered SVC, and returned to idle. No CPU trace records the required M IRQ or subsequent M return. All six terminal UART records say M finished before IRQ (0x1d). M's saved SVC frame confirms 4096 checkpoints, final index 65536, sum 2147516416, digest 18080812676020141577. Its terminal X21 is zero. No interrupted stack/X21, resumed lease, contested claim or resumed checkpoint exists to match. No physical or cross-core timing inference.

여섯 실행 모두 CPU0에서 M이 먼저 종료했다. CPU1의 P1 실행·은퇴와 유휴 복귀, CPU0의 M 복귀 및 최종 산술 결과를 원본 기록에서 확인했다. 전역 CPU1 복귀, 고정·클러스터 CPU0 복귀 모두 관측되지 않았다. CPU별 최종 카운터는 후속 복귀에서 갱신될 수 있다. 실제 보드 검증과 물리 지연 측정은 미실시다.

| Regime, launch | CPU0 last tick | CPU1 last tick | CPU0 M local ERET sample to SVC entry (ticks) |
|---|---:|---:|---:|
|Global 1|653678|724693|51034|
|Global 2|742349|800134|47380|
|Partitioned 1|432540|492775|50813|
|Partitioned 2|762663|829230|48420|
|Clustered 1|664551|743057|52274|
|Clustered 2|766706|834398|47793|

Before launch, six archived RAM windows produced identical decoded records under old and new readers. Each rebuilt ELF had a zero-initialized, aligned 16-byte counter pair. Linked CPU0 acquired slot +8 and tested failure and the four-second timeout while zero. Each return selected SP, restored exception/system and SIMD state, sampled the local counter, executed one indexed release publication, restored GPRs and executed ERET. Full disassemblies and preflight.json preserve addresses including strict M range and SGIR. The ordinary directed-wake image was separately built and its disassembly retained for static compatibility. No directed-wake guest was launched.

과거 RAM 여섯 건에서 판독 결과가 일치했다. 각 후보의 연결된 복원 경로와 CPU1 슬롯, 0 대기, 실패·시간 초과 분기를 확인했다. directed-wake 일반 이미지 호환성은 빌드와 정적 명령어 검사 범위다.

The historical strict replay CLI rejects the post-terminal QMP socket argument. Its six error outputs are retained. The independent analyze.py validates the actual frozen inputs, observer arguments, session hashes, terminal quiet and QMP stop, thread attribution, raw RAM and original exception histories, then uses the repository's terminal observation decoder. All six captures were reopened against hashes before the next launch. Per-case analysis JSON contains raw local lease intervals, CPU evidence, frame and architectural observations. Failure and timeout controls were assessed on linked instructions. No fault-injected guest was run.

[Original six model runs and independent replay / 모델 여섯 건과 독립 재생 자료](pi4-mg2-first-restore-six.zip) · SHA-256 002fd5bd17c2a2f8273722cca78caf920a29e5ca360e7cf08a16fa4fc2ecf38e.
