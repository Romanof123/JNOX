# #184 paired marker observation / 짝지은 마커 관측

Source dd034e5b8e0f145c3ab5ecb19b857127b394f9fa, plan 12d807dd723be96fe502010946818efb95fd6756. Ubuntu WSL uid 1000, distro QEMU 6.2.0, single Cortex-A72 TCG vCPU, default clock and original capture timeout. The share inputs came directly from the public third-cohort frozen plain archive. Both first cold acquisitions and both current offline replays exited 0. A shell quoting failure before either boot invoked the argument parser with missing arguments. Its diagnostics are retained as dotfiles. No guest retry, rebuild, fault injection or host scheduling change.

고정된 공개 ELF를 그대로 사용해 일반 부팅 후 관찰 부팅을 각각 한 번 실행했다. 두 캡처와 현재 도구의 두 재생 모두 성공했다. 부팅 전 명령 인용 오류는 인자 검사 단계에서 종료됐으며 원본 ZIP에 보존했다.

| Guest outcome / 게스트 결과 (ticks) | Plain / 일반 | Observed / 관찰 |
|---|---:|---:|
| IRQ service range (36) | 628040–639816 | 628330–641566 |
| Maximum gap (38 grants) | 38423 | 57149 |
| Prefix 2A−B | −608636…1279632 | −627911…1279748 |
| IRQ 36 A / B | 7566054 / 15106059 | 7570464 / 15118952 |
| Total / A fraction | 22672113 / 0.3337163 | 22689416 / 0.3336562 |

Both passed the original service, gap, prefix, total and terminal share limits. 양쪽 모두 기존 서비스·공백·누적 편차·최종 총량과 지분 예산을 충족했다.

All 152 callbacks matched QMP vCPU 0 Linux TID 959 and the 38 closed grants in raw RAM (36 IRQ, two SVC). Six linked instruction words matched disassembly and RAM. Site pairs were return 40201218/1c (MRS x17/STR), synchronous entry 40201040/44 (MRS x0/MOV), IRQ entry 40201058/5c (MRS x0/MOV). Both clock resolutions were 1 ns. All 76 adjacent widths passed, spanning 1,405–32,222 ns. Independently recomputed whole-grant wall, thread CPU and outside-CPU bounds match observed.json for every grant with the specified 1,000 ppm allowance. IRQ outside-CPU upper bounds reached 42,985 ns and 12 IRQ grants had strictly positive lower bounds. The two SVC lower bounds were zero and their upper bounds reached 3,309 ns. The prior plan-commit replay accepted plain and rejected observed with “unexpected capture file”. The current replay verified both.

152개 콜백의 실제 스레드와 QMP TID가 일치했고 원본 RAM의 전체 그랜트 순서와 대조했다. 양 시계의 해상도는 1 ns, 76개 짧은 대조 구간은 모두 1 ms 이하였다. 모든 그랜트의 벽시계·스레드 CPU·CPU 외 시간 범위는 독립 재계산과 일치했다. 12개 IRQ 그랜트는 CPU 외 시간 하한이 양수였다. 전체 그랜트에는 만료 전 실행도 포함되므로 만료 이후 지연이나 IRQ 신호 시점, 과거 실패 원인을 특정할 수 없다. 단일 쌍은 계측 영향의 일반 상한을 제공하지 않는다.

The whole-grant intervals include work before compare. They cannot locate post-deadline delay, identify IRQ assertion, explain historical misses or bound perturbation across runs. Every per-grant bound and both adjacent controls appear in original.zip, observed.json → marker_observation.intervals. Independent Windows parsing is reproducible with independent.py in this folder.

Original ZIP SHA-256 6c9c6a25c7305436523a53bcb23b4644ed29b65fa24c32d6d88a53053b46ae6b.

[First paired guest captures and independent verifier / 최초 두 실행과 독립 재생기](native-marker-share-first.zip) · SHA-256 8f2ef1925931fad23669cb6c5da963c37888e44acbc14be517c9c64382c4c162.
