# Legacy CBS first launches at d6a68bf / 기존 CBS 최초 부팅 독립 검증

The plain basic, opt-in diagnostic basic and original wake workloads each passed on their first cold launch at `d6a68bf504d0ce15a09c38220e10ff2a02c60a91`. Neither basic launch produced a diagnostic row. The earlier failure after IRQ 4 at `259aaebb` still has no identified failed comparison. [Incident #176](https://github.com/Romanof123/JNOX/issues/176) · [Implementation #187](https://github.com/Romanof123/JNOX/pull/187) · [Original failing capture](ordinary-cbs-integrated-compat.en-ko.md)

| Launch order / 최초 실행 순서 | Image SHA-256 / 이미지 해시 | Original observer / 원 관찰기 |
|---|---|---|
| 1. Plain `cbs-bench`, QEMU PID 552 | `6f8cdd32a66661d53d7134ded15bec5233fdda775608e22228a5f393468f2d17` | 0. Nine lines, five IRQs, A and B resumed, timer stop and quiet. |
| 2. `JNOX_CBS_CONDITION=1 cbs-bench`, QEMU PID 688 | `bfedb56152b92349cfe3b62531536d018f7758d4651c7930afe51b37473804ca` | 0. Same nine lines and five IRQs. No `CBS DIAG1` record. |
| 3. Plain `cbs-wake-bench`, QEMU PID 810, capture process 808 | `25082cf63245c3e8b706c0d7cb7ff90070a41d555f48058c891aef50a2ea06ba` | 0. Four lines, 34 witness rows, 78 ordered PC events, timer stop and quiet. |

## English

One native Linux cargo/rustc 1.95.0 toolchain built all four images at the same source and target paths with fresh target directories, `--locked --release -j1`, the `aarch64-unknown-none` target and the repository's Cortex-A72 flags. After the three guest positions, the same plain build at pre-selector `495a7d9d5f2514d3cef44c69be2cc90b7441094e` produced an identical complete ELF. Both PT_LOAD headers and byte streams matched, including the zero-file-size writable segment's memory extent. The diagnostic executable segment held 14,689 bytes versus 13,257 for plain. Each guest used QEMU 6.2.0, virt/GICv2, one Cortex-A72, TCG and 128 MiB. The basic observer used its five-second deadline and wake used the accepted 15 seconds. Each observed 0.1 s of terminal quiet before the host stopped QEMU. [Basic contract](../m1-cbs.en.md) · [Wake contract](../m2-cbs-wake.en.md)

Both basic captures gave 112 UART bytes and the A, B, idle, A, B grant sequence. The independent reader resolved the vector and task ranges from each loaded ELF, verified all five EL1 IRQs, and matched the last two ERETs to A's saved `0x40201278` and B's saved `0x402012fc`. On wake, independent EDF and CBS accounting matched all 34 rows before and after charging. A blocked at slots 0, 4, 8, 14 and 22 and resumed at 4, 8, 14, 22 and 23. B received nine grants and idle received 17 full slots. The remaining eight grants went to A. The 34 IRQs, five SVCs and 39 ERETs matched the journal in order. IRQs arrived 6,845–23,704 counter ticks after their compares, inside the 3,125,000-tick slot. QMP reported one running AArch64 CPU at collection. These are emulated counter observations from this launch. [Captured originals and replay](cbs-first-condition-independent.zip)

After freezing the three launches, the reader verified the eight embedded artifact hashes in EVD-029 and replayed its journal and trace. All 34 grant decisions and all 78 event kinds and addresses matched across the two recordings. The fresh PC range checks use the fresh ELF. The archived EVD-029 record supplies its original ELF hash, journal and trace. Three observer unit tests passed. The observer rejected a wrong receipt hash. Three native Cargo controls rejected the wrong selector value, wake feature and extra feature as specified. The earlier #176 failure and the eight ordinary reservation results retain their original provenance. [EVD-029](evd-029.en.md) · [Reservation integration #172](https://github.com/Romanof123/JNOX/issues/172) · [Ordinary reservation cases #123](https://github.com/Romanof123/JNOX/issues/123)

## 한국어

같은 리눅스 Rust 1.95.0 도구와 빌드 경로에서 새 타깃 디렉터리를 사용해 `--locked --release -j1`로 빌드했다. virt/GICv2, Cortex-A72 한 코어, TCG, 메모리 128 MiB에서 위 순서대로 각 이미지를 한 번씩 부팅했다. 기본 시험은 5초, 차단·깨우기 시험은 기존 15초 제한을 사용했다. 각 게스트에서 타이머 종료 표식과 0.1초 무출력을 확인한 뒤 호스트가 종료했다. 세 부팅 이후 같은 경로에서 빌드한 변경 전 커밋 `495a7d9d`와 후보의 일반 기본 ELF는 파일 전체와 두 PT_LOAD 구간이 일치했다. 진단 이미지의 실행 구간은 14,689바이트로 일반 이미지의 13,257바이트보다 컸다.

일반·진단 기본 시험 모두 UART 112바이트 9줄과 IRQ 5회가 일치했다. 독립 판독기는 각 ELF의 심볼로 작업 범위와 벡터를 확인하고 마지막 두 ERET가 A의 저장 주소 `0x40201278`, B의 저장 주소 `0x402012fc`로 복귀했음을 확인했다. 두 게스트에서 실패 조건은 관측되지 않았다. 과거 `259aaebb`에서 네 번째 IRQ 이후 발생한 실패의 조건과 원인은 여전히 미상이다. [과거 최초 실패 기록](ordinary-cbs-integrated-compat.en-ko.md)

차단·깨우기 시험은 34개 슬롯의 할당 전후 예산과 마감 시각, EDF 선택, 타이머 범위, 작업별 프레임과 진행 값을 검증했다. A는 0·4·8·14·22번 슬롯에서 차단되고 4·8·14·22·23번 슬롯에서 재개됐다. A가 8회, B가 9회, 유휴 작업이 17회 슬롯을 받았다. IRQ 34회, SVC 5회, ERET 39회의 주소와 순서가 저널과 일치했다. 세 실행 원본의 해시를 고정한 뒤 EVD-029의 원본 여덟 개 해시와 저널·예외 기록을 다시 검증했다. 두 실행의 34개 슬롯 선택 및 78개 이벤트 주소가 일치했다. 이번 에뮬레이터에서 각 IRQ 지연은 6,845~23,704 카운터 틱으로 슬롯 길이 3,125,000틱 이내였다. 관찰기 단위 시험 3건이 통과했고 잘못된 영수증 해시와 선택자 조합 3건이 거부됐다. [EVD-029 원본](evd-029.md)

## Offline replay / 보존 자료 재검증

Each launch copied its original files to the durable drive and hashed them before that WSL invocation returned. A separate invocation reopened the files and checked all hashes before the next position. The ZIP contains the ELF, build and runner logs, argv, identities, dictionary, raw UART and exception histories, wake journal and QMP state, per-position hashes, host controls and independent replay output. The first two frozen receipt SHA-256 values are `2b15fc228920eb8acbbc31f3760da38b321d1be66183c9f91f200e82dce80847` and `32ae8ed74c87f1acadcc05343cec7d2d80cef304590fefe58319fd736038f7cb`. ZIP SHA-256 is `2fea9683603a589d8cdec63295ada98a76993a9588c84e2b0f3f6ded23b79b99`. [Download raw evidence / 원본 자료](cbs-first-condition-independent.zip)

압축을 푼 디렉터리에서 pyelftools가 있는 Python 3로 다음 명령을 실행한다. `repo`는 위 후보 커밋의 체크아웃이다. Run from the extracted ZIP with `repo` pointing to the candidate checkout.

```sh
python3 replay.py . "$repo"
python3 "$repo/tools/cbs_condition_observer.py" 01-plain 2b15fc228920eb8acbbc31f3760da38b321d1be66183c9f91f200e82dce80847
python3 "$repo/tools/cbs_condition_observer.py" 02-condition 32ae8ed74c87f1acadcc05343cec7d2d80cef304590fefe58319fd736038f7cb
```
