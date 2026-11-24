# Ordinary P/Q first cohort

Fourteen of 920 windows exceeded 62,500 ticks (1 ms). All fourteen occurred at generation 2 under the default clock. Position 32 (P/D) took 77,022 ticks, 14,522 over budget. All 80 captures passed structural checks with 160 bounded exits (41 and 42 per boot).

0799f56557286d9caca8e1adec673a9b10da3a38 · Rust 1.95.0 · QEMU 6.2.0 TCG virt Cortex-A72 · 62.5 MHz · ABI v2/v3 · 1 vCPU

A/B copy frames and C/D retain frames. B/D use the cursor and A/C scan. Maxima below use ticks.

| Table | Clock | Cell | Windows | Misses | Maximum | Position | Gen 2 max | Gen 3+ max |
|---|---|---|---:|---:|---:|---:|---:|---:|
| p | default | A | 50 | 4 | 66497 | 31 | 66497 | 31593 |
| p | default | B | 50 | 3 | 72857 | 29 | 72857 | 23093 |
| p | default | C | 50 | 2 | 63220 | 55 | 63220 | 26615 |
| p | default | D | 50 | 4 | 77022 | 32 | 77022 | 25489 |
| p | icount3 | A | 50 | 0 | 1432 | 12 | 1424 | 1432 |
| p | icount3 | B | 50 | 0 | 1430 | 11 | 1426 | 1430 |
| p | icount3 | C | 50 | 0 | 737 | 9 | 727 | 737 |
| p | icount3 | D | 50 | 0 | 735 | 10 | 729 | 735 |
| q | default | A | 65 | 0 | 62005 | 46 | 62005 | 24152 |
| q | default | B | 65 | 0 | 60345 | 48 | 60345 | 24214 |
| q | default | C | 65 | 0 | 55638 | 47 | 55638 | 25757 |
| q | default | D | 65 | 1 | 72548 | 18 | 72548 | 28288 |
| q | icount3 | A | 65 | 0 | 1439 | 14 | 1429 | 1439 |
| q | icount3 | B | 65 | 0 | 1438 | 16 | 1432 | 1438 |
| q | icount3 | C | 65 | 0 | 742 | 15 | 732 | 742 |
| q | icount3 | D | 65 | 0 | 741 | 13 | 736 | 741 |

Median generation 2 effects in ticks, five matched rounds per row. contrasts.csv contains each operand and effect for every generation.

| Table | Clock | B−A | D−C | C−A | D−B | Interaction |
|---|---|---:|---:|---:|---:|---:|
| p | default | 792 | 2328 | -432 | 523 | 1536 |
| p | icount3 | 2 | 2 | -697 | -697 | 0 |
| q | default | -1660 | -21 | -3712 | -1801 | -452 |
| q | icount3 | 3 | 4 | -697 | -696 | 1 |

All 80 original stock replay invocations stopped at the missing pyelftools dependency. The archive retains every original log. A separately labeled offline replay supplied that dependency and agreed with the independent decoder on all 920 immutable window captures. Guest retries numbered zero. The fifth local block repeats the first order. Default-clock variation and five blocks limit inference about hardware, WCET, host causes and the overall release.

136 Rust tests and 11 Python selector/accounting tests passed. All eight user ELFs and probes had identical hashes. linked/ contains the ELF dispatch calls, restore vectors, symbols, cfg, section sizes and both parent diffs.

[80 boots](boots.csv) · [920 windows](windows.csv) · [14 misses](red-windows.csv) · [230 matched comparisons](contrasts.csv) · [Packages](packages.csv) · [Raw ZIP](jnox258-raw.zip)

# 일반 P/Q 최초 80회 검증

920개 중 14개 창이 62,500틱(1 ms)을 초과했다. 모두 기본 클록의 2세대이며, 가장 늦은 32번 P/D는 77,022틱으로 한도를 14,522틱 초과했다. 구조 검증은 80회 모두 통과했고 종료 41·42를 총 160회 확인했다.

0799f56557286d9caca8e1adec673a9b10da3a38 · Rust 1.95.0 · QEMU 6.2.0 TCG virt Cortex-A72 · 62.5 MHz · ABI v2/v3 · 1 vCPU

A/B는 복사, C/D는 프레임 보존. B/D는 커서, A/C는 기존 탐색. 아래 표의 최대값 단위는 틱이다.

| Table | Clock | Cell | Windows | Misses | Maximum | Position | Gen 2 max | Gen 3+ max |
|---|---|---|---:|---:|---:|---:|---:|---:|
| p | default | A | 50 | 4 | 66497 | 31 | 66497 | 31593 |
| p | default | B | 50 | 3 | 72857 | 29 | 72857 | 23093 |
| p | default | C | 50 | 2 | 63220 | 55 | 63220 | 26615 |
| p | default | D | 50 | 4 | 77022 | 32 | 77022 | 25489 |
| p | icount3 | A | 50 | 0 | 1432 | 12 | 1424 | 1432 |
| p | icount3 | B | 50 | 0 | 1430 | 11 | 1426 | 1430 |
| p | icount3 | C | 50 | 0 | 737 | 9 | 727 | 737 |
| p | icount3 | D | 50 | 0 | 735 | 10 | 729 | 735 |
| q | default | A | 65 | 0 | 62005 | 46 | 62005 | 24152 |
| q | default | B | 65 | 0 | 60345 | 48 | 60345 | 24214 |
| q | default | C | 65 | 0 | 55638 | 47 | 55638 | 25757 |
| q | default | D | 65 | 1 | 72548 | 18 | 72548 | 28288 |
| q | icount3 | A | 65 | 0 | 1439 | 14 | 1429 | 1439 |
| q | icount3 | B | 65 | 0 | 1438 | 16 | 1432 | 1438 |
| q | icount3 | C | 65 | 0 | 742 | 15 | 732 | 742 |
| q | icount3 | D | 65 | 0 | 741 | 13 | 736 | 741 |

같은 표·클록·라운드의 2세대 효과 중앙값(틱), 각 행은 5개 비교이다. 모든 세대와 개별 쌍은 contrasts.csv에 있다.

| Table | Clock | B−A | D−C | C−A | D−B | Interaction |
|---|---|---:|---:|---:|---:|---:|
| p | default | 792 | 2328 | -432 | 523 | 1536 |
| p | icount3 | 2 | 2 | -697 | -697 | 0 |
| q | default | -1660 | -21 | -3712 | -1801 | -452 |
| q | icount3 | 3 | 4 | -697 | -696 | 1 |

최초 stock replay 80건은 pyelftools 누락으로 종료했다. 최초 로그를 모두 보존하고 동일 해시의 원본에 의존성을 지정해 오프라인 판독했다. 독립 판독 결과와 920건 모두 일치했다. 게스트 재시도는 0건이다. 라운드 5는 최초 순서를 반복한다. 기본 클록의 변동 및 블록 5개로 인해 실제 하드웨어, 최악 실행 시간, 호스트 원인 또는 릴리스 전체 합격을 판정할 수 없다.

호스트 Rust 136건, 선택자 및 회계 Python 검사 11건 통과. 사용자 ELF 및 프로브는 8개 패키지에서 동일했다. 각 ELF의 dispatch 분기, 프레임 뱅크, 복원 벡터, cfg 및 섹션 증거는 linked/에 있다.

[80 boots](boots.csv) · [920 windows](windows.csv) · [14 misses](red-windows.csv) · [230 matched comparisons](contrasts.csv) · [Packages](packages.csv) · [Raw ZIP](jnox258-raw.zip)

[Original 80 cold boots, per-window margins and stable independent frame replay](native-window-factorial-first.zip) · SHA-256 6e14425522db857661182c5e064d39ef9644bb182b80f28b7b172c639645479d.
