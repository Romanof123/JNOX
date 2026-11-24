# CHG-049 cold-start diagnostics / 콜드 부팅 계측

Future opt-in cold experiment on the #97 v3 source. Record diagnostic captures separately from the existing EVD-042 observations. [Diagnostic workstream #109](https://github.com/Romanof123/JNOX/issues/109)

#97 v3 소스 기반의 선택적 콜드 부팅 실험 계획이다. 진단 기록은 기존 EVD-042 관측과 분리해 보존한다. [진단 추적 #109](https://github.com/Romanof123/JNOX/issues/109)

Single-core virtual healthy workload. Follow the paired cold matrix in the [frozen plan](https://github.com/Romanof123/JNOX/blob/d78ad8b06782997e130c1c751f6713286eb04c6f/docs/control-cold-plan.md). Retain each original stock failure. The diagnostic ELF uses the same timer, PPI, exception frame and restored idle stack. Its first counter sample B follows an ISB at the entry of the ELF. Reset-to-B is blind. PL061 readback concerns the virtual register.

Use the build command for the ordinary healthy case with JNOX_CONTROL_COLD=diagnostic, separately for control-rm and control-dm. This selector requires JNOX_PROFILE=periodic, explicit JNOX_CONTROL_CASE=healthy and JNOX_BOARD=virt. Use the *same ELF for all three modes per policy*. Supply exactly one byte of ASCII (1, 2 or 3) through the named read-only fw_cfg file. The guest checks the device signature, reads up to 256 directory entries, verifies a unique matching name, file key and size, then reads the selected byte. Missing, ambiguous or malformed input reports 4294967295 in row 48 and fails closed. Match row 48 against the predeclared mode before scoring and archive the actual input bytes, file hash, ELF hash and invocation. Keep the 96 prior sentinel captures and the 32 stock captures in their original cohorts. Reading fw_cfg adds boot work after B, which remains in the measured interval. The interface and address are specific to the frozen virtual machine. [QEMU 6.2 virt map](https://github.com/qemu/qemu/blob/v6.2.0/hw/arm/virt.c) · [fw_cfg specification](https://www.qemu.org/docs/master/specs/fw_cfg.html). Without the build selector, use the ordinary v3 startup path and invocation.

~~~sh
export JNOX_BOARD=virt JNOX_PROFILE=periodic JNOX_APP=control-rm JNOX_CONTROL_CASE=healthy JNOX_CONTROL_COLD=diagnostic
cd kernel
cargo build --release --locked
elf=target/aarch64-unknown-none/release/jnox-kernel
# Set run_dir to this attempt's new capture directory, and mode to its declared 1, 2 or 3.
case "$mode" in 1|2|3) ;; *) exit 1 ;; esac
printf '%s' "$mode" > "$run_dir/cold-mode.bin"
sha256sum "$elf" "$run_dir/cold-mode.bin" > "$run_dir/input.sha256"
# Append this argument pair to the frozen QEMU command for that capture:
# -fw_cfg "name=opt/jnox/cold-mode,file=$run_dir/cold-mode.bin"
~~~

1. Mode 1 instruments stock-style epoch selection on idle (current time plus one half phase). Capture OFF/ON with identical instrumentation. Changes to timing relative to the published v3 image must be reported.
2. Mode 2 fixes phase zero at B+250 ms. Wait with IRQ enabled on idle through B+200 ms without programming a probe compare. Require successful initialization and phase-zero arming before B+220 ms.
3. Mode 3 uses the same deadlines. Program B+200 ms exactly once, acknowledge the real timer IRQ, disable the source, EOI, and observe the PPI pending and active bits clear before the cutoff. A probe arriving >0.5 ms late still supplies a diagnostic latency if all boot limits hold. Timer observation and boot failure never count as control decisions.

The initial gate stays closed and no worker can consume input before the first operational IRQ at/after E0. Require OFF before arming and at operational transition. Boot errors disable the timer, request OFF, read it back and dump retained rows. Missing terminal output is a failed attempt under the 10 s external host timeout. Keep hash, argv, mode, row order, stdout, stderr and optional exception trace for every launch. Score every expected operational phase against E0. Row 60 or missing operational evidence precludes a successful boot. Software observations include their full elapsed cost.

| Event | Counter, argument A, argument B |
| --- | --- |
| 48–50 | B/frequency/mode; epoch planning begin/end/fixed target; idle epoch selection begin/end/operational E0 |
| 51–52 | programming begin/end/requested compare; readback end/actual CVAL/CTL |
| 53–54 | idle entry/DAIF/PPI pending-active bitmap; sample/enable bit and priority in bits 8–15/CPU PMR |
| 55–56 | vector sample/handler sample/interrupted ELR; after IAR read/full IAR/interrupted DAIF mask |
| 57–59 | probe EOI and readback end/CTL/pending-active bitmap; startup ready/E0/0; gate opened/E0/0 |
| 60–61 | failure/reason/observed output; post-unmask sample/DAIF/site (1 initial, 0 operational arm) |
| 62–64 | GIC sampling end/distributor CTLR/interface CTLR; first IRQ timer sample/CVAL/CTL; OFF read begin/end/value |

Bitmap bit 0 denotes pending, bit 1 active. Use rows 51, 52, 55 and 61 for arm/unmask margin, vector-minus-compare and handler-minus-vector. Event 55 includes vector prologue cost. Row 61 samples after unmask, so a vector before that sample bounds the unmask instant without identifying its exact tick. Use existing operational release, selection, decision, device and terminal records. Failure codes are 1 invalid selector/frequency, 2 deadline overflow, 3 arm readback/late arm, 4 idle setup, 5 cutoff/OFF, 6 stale PPI/output, 7 late operational arm, 8 IAR/mask/source state, 9 probe timing, 10 post-EOI state/cutoff, 11 operational entry/application state, 12 unexpected IRQ state and 13 initial output. The first operational phase can still fail the original timing envelope after successful initialization.

## 한국어

위 동결 계획대로 정책별 같은 계측 ELF에서 OFF/ON 콜드 쌍을 실행한다. 원본 실패와 모든 후속 실패를 분모에 남긴다. 빌드·실행 선택자는 위 명령과 같다. healthy, periodic, virt에서만 diagnostic을 허용한다. 실행별 새 기록 디렉터리에서 위의 printf 명령으로 예정 모드를 줄바꿈 없는 한 바이트 ASCII 파일로 만들고 sha256sum으로 ELF와 파일 해시를 보존한다. QEMU 명령에 `-fw_cfg "name=opt/jnox/cold-mode,file=$run_dir/cold-mode.bin"`을 추가한다. 게스트는 장치 서명과 최대 256개 디렉터리 항목에서 해당 이름의 유일성·키·길이·내용을 확인한다. 누락·불일치 입력은 사건 48에 4294967295를 남기고 차단한다. 수용 전에 사건 48의 모드와 계획된 모드의 일치를 확인하고 실제 입력 및 명령행을 기록한다. 기존 sentinel 96건과 stock 32건은 원래 집단에 보존한다. fw_cfg 읽기 비용은 B 이후 경과 시간에 포함한다. 모드 1은 idle에서 현재 시각에 반 위상을 더하는 기존 기준을 계측한다. 모드 2와 3은 모두 최초 카운터 B+250 ms를 E0로 고정하고 B+220 ms 전에 초기화 및 위상 0 설정을 끝내야 한다. 모드 2는 B+200 ms까지 시험 IRQ 없이 기다린다. 모드 3은 B+200 ms에 비교기를 한 번 설정하고 실제 운영 PPI/벡터/idle 경로에서 ACK, 타이머 해제, EOI 및 pending·active 해소를 기록한다. 자가시험 지연이 0.5 ms를 넘으면 실제 값을 보존한다. 부팅 실패에도 E0를 옮기지 않으며 재시도하지 않는다.

게이트는 E0 이후 첫 운영 IRQ까지 닫혀 있고 일반 작업은 실행되지 않는다. 최초 출력 OFF와 초기화·전환 시 실제 OFF를 확인한다. 실패는 타이머를 끄고 출력을 OFF로 요청한 뒤 읽어 사건 60과 누적 기록을 출력한다. 멈춘 게스트는 외부 호스트의 시작 후 10초 제한으로 실패 처리한다. 48~50은 최초 시각·주파수·모드와 E0 계산, 51~52는 비교기 설정·되읽기, 53~54는 idle 진입·마스크·PPI 우선순위와 PMR, 55~56은 벡터·핸들러·ELR·IAR·복원 마스크이다. 57~59는 EOI 뒤 상태·부팅 준비·게이트 개방, 60~61은 실패 원인·실제 출력과 IRQ 허용 뒤 표본, 62~64는 GIC 제어·IRQ 시점 비교값/제어·OFF 읽기 구간이다. 비트 0은 pending, 비트 1은 active이다. 원래 운영 사건과 함께 설정 여유, 벡터−비교값, 핸들러−벡터를 재구성한다. 벡터 진입 코드와 계측 비용을 포함하고 리셋부터 B까지는 미관측이다. 부팅 자가시험은 제어 판정에 포함하지 않는다. 모든 운영 위상과 독립 장치 관측을 고정 E0에 따라 평가한다.

The first mode-input procedure failed in all 96 diagnostic cold runs. [EVD-050 v1](evidence/evd-050-v1.en.md) · 첫 모드 입력 절차는 진단 콜드 실행 96회 모두 실패했다. [한국어](evidence/evd-050-v1.md)

All 128 v1 captures belong to the original sentinel-source cohort. The fw_cfg source frozen at b9db104 requires a separate cold series. Its effectiveness is unverified. [EVD-050 index](evidence/evd-050.en.md)

v1 기록 128건은 모두 원래 sentinel 소스 집단에 속한다. b9db104의 fw_cfg 입력 변경 효과는 검증되지 않았으며 별도 콜드 실행으로 평가한다. [EVD-050 색인](evidence/evd-050.md)

[Observer storage crossover, 32 boots](evidence/evd-050-storage.en.md) · [관측 파일 저장 경로 교차 실험, 32회](evidence/evd-050-storage.md)
