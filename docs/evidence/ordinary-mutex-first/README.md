# CHG-067 · Independent first cold cohort / 독립 최초 콜드 실행

Candidate 47ea89049b3efd2dd74642562a9973e15ad26a15 (#149). Plan fb7d9429e00a3a41797dac7ac85f5e5a68602915 (#148). The first PI plain launch was unqualified. The other three covered the specified guest execution. The PI pair therefore misses its two-launch acceptance condition. Both ceiling positions covered the workload. [Normative plan](../../ordinary-mutex-plan.md) · [Worker and journal source](../../../kernel/src/ordinary_mutex.rs) · [Manifests](../../../kernel/src/ordinary_mutex_manifest.rs)

최초 PI 일반 실행은 보호 계산 시작 전에 IRQ가 도착해 미적격이다. PI 추적 실행과 천장 일반·추적 실행은 요구된 동작을 관측했다. PI 구성은 두 실행 모두 충족해야 하는 조건을 달성하지 못했다. 재시도는 없었다.

| Position / 순서 | Verdict / 판정 | Decisions / restores / chunks | IRQ saved PC | Completed chunks at IRQ |
|---|---|---|---|---:|
| 01 PI plain | Unqualified / 미적격, reason 16 | 2 / 2 / 0 | 0x40202b74 | 0 |
| 02 PI traced | Covered / 충족 | 8 / 8 / 256 | 0x40202bc8 | 38 |
| 03 ceiling plain | Covered / 충족 | 13 / 13 / 256 | 0x40202cd8 | 0 |
| 04 ceiling traced | Covered / 충족 | 13 / 13 / 256 | 0x40202cd8 | 52 |

01 armed at 0x32029 with compare/readback 0x33893. LOW's assembly pre-ERET sample was 0x339ae, already 283 ticks past compare. The post-acquire task marker was 0x33e65. IRQ entry was 0x363a2, IAR 30, SPSR 0x345. LOW still owned X but the saved PC was the first prologue instruction, before the work flag and input load. The ready mask became 0xf and HIGH was selected. No matching HIGH acquire, transfer, return or committed result followed. Batch, private result and medium sum remained zero. In 03 the saved PC points to the loop's input-index instruction and the work flag was set. Zero **completed chunks** there leaves the individual sample count at IRQ unresolved.

01은 실제 복귀 직전 계측 때 이미 비교 시각보다 283틱 늦었다. IRQ의 저장 PC는 함수 첫 스택 조정 명령으로, 입력 처리 전이었다. HIGH를 선택한 뒤 실제 획득·이전·복귀·게시 기록 없이 종료됐다. 03에서는 같은 청크 수 0이라도 작업 플래그가 설정됐고 저장 PC가 실제 입력 루프에 있었다. 그 순간 처리한 개별 입력 수는 확정할 수 없다.

For 02–04, all 768 chunk rows matched (256k,2176k) for k=1…256. Each HIGH read (1,65536,557056), MEDIUM produced 32896, and each worker exited once. PI boosted LOW to 9 ahead of ready MEDIUM, then restored 2 before HIGH resumed the blocked acquire. Ceiling LOW reached 5 then 9 before contention; its midpoint yield admitted HIGH. LOW retained 5 after handing Y to HIGH, audited (1,256) under X, then restored 2. MEDIUM acquired X before reading the audit. All owner, wait, ready, priority, frame-range and saved return records were compared. On 02 and 04 the external IRQ/SVC and ERET PCs matched the ledgers. The final exit selected idle and stopped to print.

02~04는 청크 768개, HIGH 스냅샷, MEDIUM 합, 작업별 1회 종료를 확인했다. PI는 LOW의 9 상속과 2 복원, 천장은 LOW의 5→9 상승·Y 이전 후 5 유지·X 해제 후 2 복원을 확인했다. 두 천장 실행 모두 128청크 양보 뒤 HIGH가 대기했고 MEDIUM은 X를 획득한 뒤 감사 기록을 읽었다.

## Sampled intervals / 계측 구간

Each tick is 16 ns at 62.5 MHz. The one-shot offset in all four launches was exactly 6250 ticks. Columns give IRQ entry to HIGH's pre-acquire marker / SVC entry, block decision to transfer marker, transfer marker to successful task-side return marker, and LOW's post-final-acquire to commit marker. These are sampled guest intervals including observer instructions. The two attempt endpoints bracket the instruction issuing the request. The enclosed replay output also gives ownership residency lower/upper estimates from acquire/transfer and release entry/decision samples per owner and resource. 01 has no complete intervals. Physical maximum blocking and WCET remain open.

각 틱은 16 ns이며 100 µs 자극은 6250틱이다. 표는 IRQ 진입→HIGH 획득 직전/서비스 진입, 차단 결정→이전 표식, 이전→실제 작업 복귀 표식, LOW 마지막 획득 복귀→게시 표식 순이다. 표식 사이에는 관측 비용이 포함된다. 자원·소유자별 점유 구간의 하한·상한은 압축 파일의 replay.txt에 있다.

| Position | IRQ→pre / entry | Block→transfer | Transfer→return | Acquire→commit |
|---|---:|---:|---:|---:|
| 02 | 8309 / 8744 | 20055 | 1773 | 36858 |
| 03 | 16260 / 16296 | 25127 | 1542 | 42406 |
| 04 | 18410 / 18781 | 15852 | 1444 | 40929 |

## Reproduction / 오프라인 재생

Extract the archive into a new directory, then run the adjacent replay with standard Python 3 and assertions enabled. It checks the per-file SHA-256 manifest before replay. The archive contains the frozen protocol, original capture and replay scripts, compiler/build and selector logs, all four complete UART captures, ELF/disassembly/symbols, invocation and termination metadata, and the two external exception logs. Every launch stopped at host request after the UART footer within its 15 s allowance. QEMU 6.2.0 (Debian 1:6.2+dfsg-2ubuntu6.31), native Linux ext4, one Cortex-A72 virt/GICv2/TCG CPU, 128 MiB. Windows rustc/cargo 1.95.0, aarch64-unknown-none, cortex-a72, release/locked/-j1, JNOX_BOARD=virt, JNOX_PROFILE=mutex and the per-case JNOX_APP. All 131 native tests passed. Six inappropriate selectors were rejected (missing/unknown app, Pi 4, competing mutex bench, fixed trace, control case). The legacy mutex benchmark build succeeded.

새 디렉터리에 압축을 풀고 Python 3 기본 모드로 아래 명령을 실행한다. 모든 원본의 해시를 확인한 뒤 원본 저널을 재생한다. 네이티브 테스트 131개 통과, 잘못된 선택 6건 거부, 기존 벤치마크 빌드 성공을 기록했다. 원본 네 건 모두 종료 표식 후 호스트가 정지했다.

```shell
python -m zipfile -e captures.zip first-cohort
python replay.py first-cohort
```

Archive SHA-256: 38bf0de1b42c36eedd7bf90bd7e648513849c2b6bee4f9de2a3428c9083f82c1. PI ELF: 100c56a95675c2295f98067a063091139e2cbfa75fb55b52a724f8652fbce9be. Ceiling ELF: 697f563f65e87c6b6bce194589392241e40dae11ff1e663e8d9236bbd638d1f9. [Capture archive](captures.zip) · [Portable replay](replay.py)
