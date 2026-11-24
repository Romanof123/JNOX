# #218 / #217 independent verification · 독립 검증

Source and linked guard ordering verified. Directed runtime closure remains open. The original healthy #94 qualification remains open. 소스와 링크된 명령에서 보호구간 복원 순서를 확인했다. 지시된 누락 경로의 실행 검증과 #94 정상 운용 판정은 보류한다.

Candidate 97e317f786cc9d64b7cf0b83325472b7dd2e0f75, parent ca0b3dd3f87f0f459449271d957af1eb8e77d2cd, prospective plan 38aeeaa51a67b7e50d361c1650f94870976c6b3c. Windows Rust 1.95.0 locked release, single job. QEMU 6.2 virt/GICv2 TCG, one Cortex-A72, 128 MiB, native WSL ext4 guest output, passive exception and PL061 tracing. All 32 builds, 16 first boots and subsequent review were serialized. No retries or guest changes. Freeze files record commands and hashes. Both named cold-mode fixtures read ASCII 1 (row 48 mode 1), completed 26 decisions and reached CONTROL END. Generic observer labels cold incomplete; diagnostic input smoke is scored from original UART. 후보와 기준 16쌍의 진입점과 .data 내용·주소·크기가 같았으며, 모든 쌍의 PT_LOAD가 달랐다. ELF 비교표에 개별 적재 해시 및 할당 섹션을 기록했다. All sixteen selector pairs retain entry and .data and differ in load payload. No fresh parent boot was included in the prospectively ordered candidate series.

In the candidate RM descriptor, the second guard saves full DAIF to x28, conditionally takes the missed-index return, samples the end, updates the tuple maximum and restores x28 (40202f08) before testing the boolean and calling final_check (40202f40). The latter calls its own locked function and then executes SVC2 (40201a7c). The original parent branches to final_check before the second guard can reach its end sample. The candidate preserves both pending reads, the final take, real check, row 10 and stopping flag; the handler redirects the saved ELR on the same frame (40202098). No intervening worker yield or state write. A pre-masked input is preserved by full-register restoration in the linked code. The max uses lexicographic (duration,begin,end), including tie-breaking. Korean: 두 번째 검사에서 누락을 발견하면 불리언을 반환한다. 종료 카운터 측정·최댓값 회계·DAIF 전체 복원 뒤 최종 검사로 진입한다. 그 안에서 한 번 소비·판정·완료 기록 후 정지한다. 실제 저장 프레임 리디렉션 소스도 그대로다.

No passive UART requested/actual IRQ-index mismatch appeared in any first capture. A0 produced 26 normal checks in each policy. The initial-clear pending case, initial-masked control and post-decision saved-frame redirect were not observed in this candidate. Corresponding pre-SVC DAIF, worker SP/frame continuity, exact once-only terminal consumer/target, per-closure begin/end/max updates, and paired marker boundaries remain unverified dynamically for both exact binaries. Prior #213 recorded directed states in a different candidate (689a4c62); they cannot close #218. Ordinary row 33 is a global maximum only. 이번 최초 관측에 누락 IRQ가 없어 지정 경로에서 SVC 직전 DAIF, 스택·프레임, 실제 단일 소비와 목표, 각 구간 회계와 마커 쌍을 판정할 수 없었다. #213의 다른 바이너리 자료는 분리했다. No debugger or injected states were used here.

|First A0 / 최초 A0 (ticks)|RM|DM|
|---|---:|---:|
|First service / 첫 서비스|37776|15317|
|First IRQ lateness / 첫 IRQ 지연|20244|16285|
|First phase delay / 첫 위상 지연|85927|56115|
|Longest consecutive completion gap / 최대 완료 간격|167053|160979|
|Longest consecutive decision gap / 최대 판정 간격|167052|160326|
|First issue gap / 첫 발행 간격|520786|576803|
|Issued / accepted / command / rearm|7 / 0 / 0 / 0|7 / 7 / 1 / 0|
|Original first failed criterion / 최초 실패|phase envelope|rearm issue spacing|

At 62.5 MHz the original 5 ms spacing limit is 312500 ticks. Both measured spacing maxima fit. Both A0 runs failed full healthy qualification (0/2); original service 250 µs, IRQ 0.5 ms, phase 1 ms, exact issue spacing 10 ms and fault bounds retained. A1/A2 operational RM and DM all refused before worker admission with missing mask 4095 and zero issues. Each of four deterministic A1/A2 OFF boots actually issued/accepted 7/7 and received one command at the diagnostic horizon, first gap 625000, no rearm. Each of six stock, marker, valid cold smokes closed OFF; none rearmed. Every first boot recorded initial output zero and independent masked PL061 data readback zero, with no rising edge. 25/5/30 ms fault response from an energized state, physical WCET/FTTI and product suitability are unmeasured.

62.5 MHz 기준 5 ms 간격 한도는 312500틱으로 두 A0의 완료·판정 최대 간격은 이 안에 있었다. 전체 정상 통과는 0/2였다. RM은 첫 위상 지연, DM은 첫 허가 간격이 기존 판정에서 실패했다. 운영 A1/A2 네 건은 누락 마스크 4095로 작업 전 거부됐다. 결정적 OFF 네 건은 각 7/7 허가·명령 1건·재무장 0건이었다. 여섯 기존 구성 스모크를 포함한 16회 모두 독립 PL061 판독이 OFF였고 상승 에지가 없었다. ON 이후 25/5/30 ms 고장 대응 및 실기·제품 WCET/FTTI는 미측정이다.

Raw UART, exception/GPIO traces, stderr, invocation and source hash receipts are in raw/, freeze*.json. images/ contains every actual ELF, verbose build log, disassembly and symbol table. summary.json, passive.json, elf-comparison.json, legacy-loads.json and per-run original-gate.json retain detailed results. 한국어: 각 최초 UART·예외·GPIO 원본과 ELF, 디스어셈블리, 해시, 세부 판정은 같은 압축 파일에 보존했다.

[First observations and original ELFs / 최초 관측과 원본 ELF](control-guard-exit-first.zip) · SHA-256 4c1f78076b3919774d92d235b4f7383c03666fa3ba80d904b1cd8f994b8dbe4c.
