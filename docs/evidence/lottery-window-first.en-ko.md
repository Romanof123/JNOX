# PR 270 independent first verification / 독립 최초 검증

Candidate 602b1896286b9eb25b956ca1d658732441e1b719. Fresh WSL ext4 checkout and per-position release builds using locked Rust 1.95.0, one build job. The public plan defined the prospective inputs. Raw capture preceded inspection of implementation and baseline outcomes. Source status, ELF, SHA, UART, disassembly, exception trail, argv and compiler logs accompany each actual launch. Nine actual Cortex-A72 QEMU virt/GICv2 TCG launches. No physical-board measurement.

| selector | windows / minima | first result |
|---|---|---|
| lottery-window1 | 25 / 5,3,11 | 48 receipts, totals 11,14,23 |
| lottery-window2 | 25 / 1,7,10 | 48 receipts, totals 8,17,23 |
| lottery-window-tight | 25 / 5,3,11 | first short [24,48), ID 0, 5 < 6 |

Each admitted boot produced one initial idle IRQ plus 48 slot IRQs. Independently matched all external ERET/ELR PCs against selected and interrupted frames, private stack range, saved register fingerprint on subsequent selects, and PPI30. Recomputed every SplitMix word and ticket, every half-open window, each worker's 16-byte block, per-stream ordinal and all three progress snapshots at each slot. No rejection occurred in the declared seeds. See independent.json per case. Tight UART recorded only tags 15–18 and zero END; the external interrupt trace is empty. Gate-reported live state equals the cold seed before and after all three preflights. The runtime state is reconstructed from the live rows for admitted boots.

Physical counter frequency was 62,500,000 Hz and each programmed duration 625,000 counts. Across actual task slots, entry-to-vector elapsed counts were 628409–631036 / 628622–630987 for seeds 1/2. Start-to-pre-ERET costs were 221–3918 / 219–6913 and compare-to-vector lateness 3409–6036 / 3622–5987. These are sampled guest brackets, including scheduling and exception overhead, with no per-instruction worker-time or WCET claim.

The separate baseline ZIP was SHA-verified against 5e21b68e1d7f0d6c897873ce459b44423370fbf640666e6a5f096b52cab2b61e after sealing first verdicts. All six full ELF hashes differ. Five ordinary flat images also differ. The bench flat image matches exactly. All five ordinary controls were freshly booted and scored against actual UART, ELF symbols and external exceptions via the baseline independent scorer. Both ordinary lottery seeds, stride and stride-delay qualified. Lottery-miss retained first failure 496/3/0/2/1 versus baseline 446/3/0/2/1, the same measured two ticks against one at slot zero. Bench printed PROPORTIONAL PASS. The verifier initially invoked the bench incorrectly as a profile; this prelaunch error 101 is preserved separately from the correct feature's first boot. See comparison.json for each exact baseline and candidate hash, baseline first observation, and each control-score.json for candidate measurements.

Existing scheduling tests passed 139/139. Independent Python arithmetic compared with 8,640 actual Rust policy invocations, including 152 valid inputs, invalid bounds, partial and foreign masks, floor and weight failures, W=1, W=H, exact equality and tightening. The original local verifier bundle includes a Git source archive, independent helpers and first-file manifests.

한국어. 두 수용 사례에서 슬롯 48개씩, 24슬롯 창 25개씩, 실제 16바이트 작업 결과와 모든 타이머 IRQ 및 예외 복귀 주소를 독립 대조했다. 강화 사례는 마지막 창에서 ID 0이 요구한 6회에 1회 부족했고 작업이나 IRQ 없이 거절됐다. 분석 전후 기록된 실제 상태는 시드와 같았다. 모든 시간 범위는 이 QEMU 실행의 카운터 관측이다. 기존 다섯 일반 모드와 어셈블리 벤치를 재부팅했다. 의도된 lottery-miss는 기준본과 같은 슬롯에서 2틱 대 1틱 실패를 보였다. 잘못된 벤치 프로파일을 사용한 빌드 실패 원본도 보존했다. 여섯 ELF 전체 해시는 모두 달랐으나 벤치의 평탄화 바이너리는 일치했다. 기존 호스트 테스트 139개와 실제 정책에 대한 독립 입력 8,640개를 검증했다.

## Publication / 공개 원본

The published package retains all 109 other members of the original verifier ZIP byte-for-byte, including all 94 sealed first files, all nine launched ELFs, captures, host executables and independent checkers. Its source-provenance.json identifies the original 46,002,669-byte repository snapshot. Git 2.34.1 on WSL regenerated that snapshot with the exact archived digest b346ff2d819c7efe7d83caf37d9f0e872def5912bb0395a6f88ffbabb7efe379 using the command below. The complete original verifier ZIP is retained with SHA-256 767329c9c782ddf6ba508b20f9a65ed763a593a8d3d2758352fa08294d1e587d.

```sh
git archive --format=tar.gz --output=source.tar.gz 602b1896286b9eb25b956ca1d658732441e1b719
```

최초 봉인 파일 94개를 포함한 109개 항목은 원래 ZIP과 바이트 단위로 일치한다. 별도 출처 레코드가 가리키는 전체 저장소 스냅샷은 위 명령으로 동일한 해시를 재현했다. [Original six baseline images and capture / 기존 여섯 이미지 및 원본](schedule-trace-lottery-baseline.en-ko.md).

[First records, ELF images and independent checks / 최초 증거·이미지·독립 검산](lottery-window-first.zip) · SHA-256 74af552b30803ec752c68c9afa8acb9a1a16403a16227e5bed07c20e3c9745bb.
