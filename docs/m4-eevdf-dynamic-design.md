# POL-012 live configuration / 실행 중 설정 변경

CHG-041 · IMP-039 · planned EVD-039 · #85

One vCPU executes 256 consecutive one-unit physical timer grants (CNTFRQ/100 ticks, integer division). A/B stay runnable. A/B/C initially have weights 2/2/1 and slices 1/1/1. After each of its first 20 grants C executes SVC #1. After 1,2,3,4 intervening grants cyclically the scheduler wakes C. One policy and one cumulative service account span every transition.

단일 vCPU에서 CNTFRQ/100(정수 나눗셈) 틱의 타이머 할당 256회를 이어간다. 초기 A/B/C 가중치는 2/2/1, 슬라이스는 1/1/1이다. A/B는 계속 준비 상태이고 C는 처음 20회 할당 직후 SVC #1로 차단된다. 다른 태스크의 할당 1·2·3·4회(순환) 후 기상한다. 정책과 서비스 누적값은 전체 구간에서 유지한다.

Immediately before sleeps 5,9,13,17 (one-based), C issues SVC #2 targeting A with (weight,slice) arguments (2,3), (0,3), (3,3), (3,0). Accept the first and third requests, replacing A’s request at its lag-preserving rounded runtime. Reject zero weight and zero slice with every tuple and reference intact. B/C retain initial settings. Use eligible earliest deadline and lower-ID tie breaking throughout.

C는 5·9·13·17번째 차단 직전에 A를 대상으로 가중치와 슬라이스 (2,3), (0,3), (3,3), (3,0)을 레지스터로 전달해 SVC #2를 호출한다. 첫 번째와 세 번째 요청을 승인하고 기존 정책의 지연 보존 및 올림 규칙으로 A의 요청을 교체한다. 가중치 또는 슬라이스 0은 거부하고 모든 상태와 기준시각을 보존한다. B/C 설정과 적격 태스크 중 최소 마감시각, 동률 시 낮은 ID 우선 규칙을 유지한다.

For every prefix and task compare charged service with cumulative runnable weight fractions, exactly in sixtieths. Across 768 absolute errors, nearest-rank p50/p95/p99/max must each be ≤20 quanta. Across 20 wakes, the same four ranks must each be ≤8 intervening grants and ≤10 physical quanta from wake to first resumed worker timestamp. These are local fixture limits. Replay every row and update from ELF symbols, checking exception trace, PC, frame, arguments, pre/post tuples, reference, admission and subsequent selections.

각 접두 구간과 태스크에 대해 실제 서비스와 매 할당 준비 가중치 비율로 누적한 이상 서비스의 차이를 1/60퀀텀 단위로 계산한다. 절댓값 768개의 최근접 순위 p50/p95/p99/최댓값은 각각 20퀀텀 이하이다. 20회 기상의 같은 네 통계는 각각 중간 할당 8회 이하 및 기상부터 태스크 코드 재개까지 물리 시간 10퀀텀 이하이다. 유한 시험의 기준이다. ELF 심벌 원자료에서 모든 할당과 변경을 재구성하고 예외 추적, PC, 프레임, 인자, 전후 상태, 기준시각, 승인 여부와 후속 선택을 확인한다.

## Observer ABI / 관측 데이터

Read little-endian 64-bit words from the ELF symbols after the terminal marker. The eevdf_witness header and row/wake layouts match EVD-034; its row error words are signed sixtieths and its header peak is an unsigned absolute numerator over 60. The eevdf_updates symbol has four records of 37 words each. Words 0–12 are one-based sleep number, next zero-based grant index, trap counter timestamp, return PC, frame address, task ID, requested weight, requested slice, accepted (0/1), pre-reference numerator/denominator and post-reference numerator/denominator. Words 13–24 and 25–36 are the pre/post A/B/C tuples (runtime numerator, deadline numerator, weight denominator, remaining service). ee_c_config_svc and ee_c_config_return identify the linked request and return instructions.

종료 마커 뒤 ELF 심벌에서 리틀엔디언 64비트 워드를 읽는다. eevdf_witness의 헤더·행·기상 구조는 EVD-034와 같고 행의 부호 있는 오차와 헤더 최대 절댓값의 분모는 60이다. eevdf_updates는 37워드씩 네 레코드이다. 처음 13워드는 차단 순번(1부터), 다음 할당 인덱스(0부터), 트랩 시각, 복귀 PC, 프레임 주소, 대상 ID, 요청 가중치·슬라이스, 승인 비트, 전후 기준시각의 분자·분모이다. 이어지는 12워드씩은 변경 전후 A/B/C 상태(실행시각 분자, 마감시각 분자, 가중치 분모, 남은 서비스)이다. ee_c_config_svc와 ee_c_config_return은 요청 및 복귀 명령의 주소이다.

## Build and capture / 빌드와 캡처

Run from the repository root on Linux with the kernel toolchain, QEMU and AArch64 GNU binutils. The host UART deadline is 30 seconds for 256 grants. The boot command checks the full transcript. The following capture check verifies the named journal ranges and sizes. The independent policy replay uses both originals and int.trace.

Linux 저장소 루트에서 커널 툴체인, QEMU와 AArch64 GNU binutils로 실행한다. 할당 256회의 호스트 UART 대기 한도는 30초이다. 부팅 명령은 전체 UART를 검사하고 이어서 심벌의 주소 범위와 원본 크기를 검사한다. 정책 독립 재구성은 두 원본과 int.trace를 사용한다.

```sh
set -eu
out=$(mktemp -d)
(cd kernel && env -u JNOX_PROFILE -u JNOX_APP -u JNOX_FIXED_TRACE JNOX_BOARD=virt CARGO_TARGET_DIR="$out/target" cargo build --locked --release --features eevdf-dynamic-bench -j1)
elf="$out/target/aarch64-unknown-none/release/jnox-kernel"
export EEVDF_CAPTURE_DIR="$out/raw"
python3 tools/qemu_boot.py --kernel "$elf" --profile eevdf-dynamic --qemu tools/eevdf_capture.py --runs 1 --timeout 30
python3 - "$elf" "$EEVDF_CAPTURE_DIR" <<'PY'
import json, subprocess, sys
from pathlib import Path
elf, raw = Path(sys.argv[1]), Path(sys.argv[2])
launch = json.loads((raw / "launch.json").read_text())
symbols = subprocess.check_output(["aarch64-linux-gnu-nm", "-S", str(elf)], text=True)
for key, name, length in [("audit", "eevdf_witness", 64 + 256*312 + 20*96),
                          ("trace", "eevdf_updates", 4*37*8)]:
    address, size, _, _ = next(s.split() for s in symbols.splitlines() if s.endswith(" " + name))
    assert int(address, 16) == launch[key + "_address"]
    assert int(size, 16) == launch[key + "_size"] == (raw / (key + ".bin")).stat().st_size == length
assert launch["trace_symbol"] == "eevdf_updates"
PY
```

[English requirements](requirements.en.md) · [Architecture](architecture.en.md) · [Verification](verification-plan.en.md) · [Release cases](scheduling-release-cases.en.md) · [Fixed-settings evidence](evidence/evd-034.en.md)

[한국어 요구사항](requirements.md) · [아키텍처](architecture.md) · [검증](verification-plan.md) · [릴리스 사례](scheduling-release-cases.md) · [고정 설정 근거](evidence/evd-034.md)
