# JNOX #251 independent first boots / 독립 최초 부팅
Old ccbfd4db36c00de8ad720e4b9d1627cd5b632bb3, new 55ce22d949095a90438586412710d094b6f03886. Rust 1.95.0, WSL Ubuntu, QEMU 6.2.0 Cortex-A72 TCG, one CPU, interrupt logging. Four ordinary packages built serially. For each policy/clock the five old/new pairs alternated order starting old. All 40 cold first attempts reached END, structurally replayed, and retained all raw files. The identical user ELF within each policy was checked before boot. P has 10 windows and Q has 13 including the extension, followed by two bounded exit drains (41,42). Full frame bank, address isolation, repeated PID, release, idle/blocked selection, absolute compare and terminal result checks ran in the pinned replay.

모든 최초 세션에서 구조 검증과 정상 종료 두 건이 확인됐다. 기준은 62.5 MHz에서 62,500틱이며 초과 세대는 그대로 실패 처리했다. 다음 표에서 실패 부팅과 초과 창 수를 구분한다. 세대별 구간과 여유는 transitions.csv에 있다. 최초 세대에는 이전 IRQ가 없다.

|Policy/clock/build|Failed boots / 5|Missed windows / total|Gen 2 ticks in run order|Gen ≥3 max|Worst|Worst margin|
|---|---:|---:|---|---:|---:|---:|
|p-default-new|3|3/50|70944, 61921, 78799, 68441, 59528|25394|78799|-16299|
|p-default-old|5|5/50|70645, 63319, 63374, 73615, 68988|24985|73615|-11115|
|p-icount3-new|0|0/50|1426, 1426, 1426, 1426, 1426|1430|1430|61070|
|p-icount3-old|0|0/50|1424, 1424, 1424, 1424, 1424|1432|1432|61068|
|q-default-new|0|0/65|55801, 55671, 53617, 57412, 51294|29493|57412|5088|
|q-default-old|3|3/65|56125, 62704, 53578, 66379, 64989|25054|66379|-3879|
|q-icount3-new|0|0/65|1432, 1432, 1432, 1432, 1432|1438|1438|61062|
|q-icount3-old|0|0/65|1429, 1429, 1429, 1429, 1429|1439|1439|61061|

The cursor bypasses the baseline phase search on equality. Default-clock misses persist. Each report includes both worker exits and every window’s timing. All four phase comparisons and five weighted/startup control replays returned zero. The two phase-package host tests and all 136 single-threaded Rust policy tests passed. The actual cursor tests compared arbitrary seeks and successive openings for 729 tables and four readiness masks, recorded callback identity/count, and checked exact MAX, adjacent/seek overflow, and recovery. Discontinuity and overflow were exercised in Rust on the host. Per-PC branch execution was inferred from linked code and ordinary offsets. QEMU recorded interrupts and ERET rather than every instruction.

기본 클록의 지연 초과가 지속된다. 호스트 Rust 테스트는 순환·비연속·오버플로와 복구 후의 결과 및 준비 상태 조회를 검증했다. 실제 게스트 증거는 정상 창 전환·프레임·결과와 타이밍을 검증한다. 명령별 분기는 링크된 코드의 정적 분석 결과다.

## Linked AArch64 paths / 링크 코드 경로

All four full disassemblies are included. Addresses below are hexadecimal. P old/new dispatch bases 40202f90/40203358, Q 40202fc0/40203388. At dispatch+30 TBZ sends ordinary work to +60, drain falls through +34 then branches at +5c straight to +fc. Ordinary P calls its helper at dispatch+9c and Q at +b4. On return +b8 checks the discriminant, +dc..+ec selects owner/runnable (idle default 2), +f4 adds the epoch with a carry check and +110 switches the MMU.

Old helper bases P 40202978 and Q 402029f8. At +0c/+10 UDIV/MSUB computes phase, +20 loads the candidate end, +2c/+30 compares and loops with 24-byte stride. +34/+38 checks the absolute relative-end addition. New helper bases P 402016a0 and Q 402017a8. At +08/+0c compare relative opening and expected opening. Equality falls through +10 loading the index. Index zero takes +14 to +74 for zero start; nonzero valid index takes +2c..+38 to load the previous end. Both paths go to +78..+84 for the indexed current end and remaining duration. +88/+8c checks addition carry before committing. +9c..+b8 derives readiness, +bc/+c0 wraps the last index to zero or increments, +c4 stores next index/expected tick. The +0c mismatch goes to +3c, then +48/+4c UDIV/MSUB and +54..+6c slot comparison loop, rejoining +78. No divide or reciprocal multiply occurs along the equality path; MADD there indexes a 24-byte slot. Overflow returns the absent discriminant before cursor commit. The shared checks after rejoin preserve output selection and bounds.

일치 경로는 슬롯 인덱스로 시작과 끝을 읽고 carry를 검사한다. 마지막 인덱스는 0으로 돌아간다. 불일치 경로만 나눗셈과 슬롯 탐색을 거친다. 종료 드레인은 결정 함수를 건너뛴다. 각 ELF와 해시는 아래에 있다.

- build-p-new 26f2010944f759c830968d59916f02254ff210324c2a3250845b2c7deec91143
- build-p-old 69eecfd9cc84c9f4e463f7339d468d395db29281accf80c7bf3629f2efc69672
- build-q-new 4b6a09427d508f20304b81800d2836d3a5ae74f1dd5834cbf1712a1a2ffbd570
- build-q-old 6381e63ee0faa6b335c14543676b3ac6fd0bd1ee728beba1ae9f44ff6b724eeb

Replay with the corresponding pinned checkout: python3 tools/native_partition_replay.py EVIDENCE_DIR SESSION_SHA256 (digest file beside each directory). Code 1 with a JSON timing_misses list records a bound failure.

[All 40 first captures, linked paths, timing results and reference controls](native-cyclic-adjacent-cursor-first.zip) · SHA-256 b63ae650981b5cd690e0e7d4bd921c04a2efc60f84d2afed7126ad1662fe4a7f.
