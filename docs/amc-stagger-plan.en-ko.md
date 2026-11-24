# POL-015 / #234 prospective plan · 사전 계획

Freeze the first cold run independently for each of exact-lo, exact-hi, over-hi and stagger-idle. Keep ELF, map, raw UART, commands, hashes and failure or timeout from position one. An independent checker must derive the expected jobs, deadlines, counter-grid comparisons, ring sums and first failing row before reporting results. Recheck the three original schemas with their original replay. No implementation or measurement results are claimed here.

네 경우를 각각 독립적으로 최초 콜드 실행하고 실패와 시간 초과도 보존한다. 기존 세 경우의 원래 출력과 판정기를 유지한다. 원본 ELF, 맵, UART와 해시를 보존하고 별도 판정으로 릴리스, 절대 마감, 타이머, 작업 누적값을 재구성한다.

| Hazard / 위험 | Planned requirement and witness / 요구 및 관측 |
|---|---|
| hidden time or reset / 시간·모드 변경 | Explicit checked forward advance only while quiescent. Reject ready/running work and overflow. Idle queries preserve time. HI mode survives idle. |
| shifted deadlines / 마감 이동 | One counter origin and fixed quantum. At 1 enter WFI, at 2 retire physical timer IRQ. Release LO at 2, surviving HI at 3, triggering HI at 4. Deadlines use the original grid. |
| lost carry-in / 잔여 작업 손실 | At 5 discard the partially serviced LO generation. Retain partially serviced HI work across transition. Verify generation, frame continuity and checksum progress at each grant. |
| unsupported approval / 승인 오판 | Report observed response counters, including finite completion, separately from ideal slot results. Enumerate finite bounded idle offsets and carry-in demands separately. External product limits and target evidence remain external inputs. |

측정 응답시간은 승인 한계로 쓰지 않는다. 유한 모델의 입력 범위와 실제 게스트 한 경우를 구분한다.

## Implementation and independent acceptance / 구현 및 독립 판정

### Corrected public build selector / 공개 빌드 선택기 정정

The first instruction was `JNOX_AMC_APP=stagger-idle`. Independent QA recorded Cargo exit 101 before guest launch. The build script reads `JNOX_APP` and emits `JNOX_AMC_APP` for the compiler after validating the board, profile and case. QA reached all four controls using the public selector. [#234](https://github.com/Romanof123/JNOX/issues/234) · [#237](https://github.com/Romanof123/JNOX/pull/237)

최초 지침은 `JNOX_AMC_APP=stagger-idle`였고 독립 QA에서 게스트 실행 전 Cargo 종료 코드 101을 기록했다. 호출자가 설정할 값은 `JNOX_APP`이다. 빌드 스크립트가 보드·프로필·케이스를 확인한 뒤 컴파일러에 내부 변수 `JNOX_AMC_APP`를 전달한다. QA는 공개 선택기로 네 대조 사례에 도달했다.

Run from the repository root in Bash with the AArch64 bare-metal Rust target, Python with pyelftools and QEMU on PATH. Start with a clean selector environment. The example defaults to the public tree-identical source and replay commit from PR #237 (fetch its PR head if missing), supports a prospective combined revision via JNOX_REPLAY_REVISION, creates a fresh cohort, and places linker maps and trusted digests outside the case directories. Preserve a failed first session and replay its digest.

저장소 루트의 Bash에서 AArch64 베어메탈 Rust 타깃, pyelftools가 설치된 Python, PATH에 있는 QEMU를 사용한다. 선택기 환경을 비우고 다음 명령으로 원본과 트리가 동일한 PR #237의 공개 커밋에 고정한 소스와 판정기를 사용한다. 이 커밋이 없으면 아래 명령이 공개 PR 헤드를 가져온다. 통합 리비전을 사용하려면 JNOX_REPLAY_REVISION에 해당 커밋의 전체 해시를 지정하고 해당 브랜치를 먼저 가져온다. 각 경우의 링크 맵과 다이제스트는 별도로 보관한다. 최초 실행 실패도 해당 세션 다이제스트로 재생하고 보존한다.

```bash
set -eu
public_revision=0cc2e63c1dc5ba10b4103237846b180c295417cb
revision=${JNOX_REPLAY_REVISION:-$public_revision}
# For an override, fetch its integration branch first / 통합 브랜치는 먼저 가져온다.
if [ "$revision" = "$public_revision" ] && ! git cat-file -e "$revision^{commit}" 2>/dev/null; then
    git fetch https://github.com/Romanof123/JNOX.git refs/pull/237/head
fi
revision=$(git rev-parse --verify "$revision^{commit}")
out=$(mktemp -d)
git worktree add --detach "$out/source" "$revision"
cd "$out/source"
qemu=$(command -v qemu-system-aarch64)
export CARGO_TARGET_DIR="$out/target"
status=0
for app in exact-lo exact-hi over-hi stagger-idle; do
    (cd kernel && JNOX_BOARD=virt JNOX_PROFILE=amc JNOX_APP="$app" \
        cargo rustc --release --locked -j1 -- -C "link-arg=-Map=$out/$app.map")
    elf="$CARGO_TARGET_DIR/aarch64-unknown-none/release/jnox-kernel"
    python3 tools/ordinary_amc_capture.py freeze "$app" "$out/$app" "$elf" "$qemu" "$revision" > "$out/$app.freeze"
    python3 tools/ordinary_amc_capture.py run "$out/$app" "$(cat "$out/$app.freeze")" > "$out/$app.session" || status=1
    python3 tools/ordinary_amc_replay.py "$out/$app" "$(cat "$out/$app.session")" > "$out/$app.replay.json" || status=1
done
exit "$status"
```

The original three selectors retain wire versions 1/2 and their acceptance paths. 기존 세 선택기의 버전 1/2 출력 및 판정 기준은 그대로 유지한다.

The host policy test enumerates all 16 tuples of idle length 1–4 and LO and surviving HI demand 1–2 each. The triggering HI demand is fixed at 2. The guest exercises only (1,2,2). At boundaries 0–6 expect task/generation 0/0, idle, 0/1, 3/0, 1/0, 1/0, 3/0. The transition at 5 discards 0/1 with one grant used. The resumed finite 3/0 reduces its second ring and issues SVC; the original absolute deadline is 8. Report its measured SVC entry minus the scheduled release counter at boundary 3. Other completed service jobs report retirement samples separately. The checker matches private stacks, linked PCs, absolute compares, ring reductions, final products and same-process exception order. External product acceptance requires its own workload admission, hardware evidence and approved limits.

호스트의 16개 조합은 유한 정책 모델이며 실제 실행 측정은 (1,2,2) 한 경우다. 유한 작업의 완료 응답은 경계 3에서 SVC 진입까지의 실제 카운터 차이다. 다른 작업의 슬롯 종료 관측값과 구분한다. 제품 승인 한계와 실제 대상 장치 근거는 별도 입력이다.
