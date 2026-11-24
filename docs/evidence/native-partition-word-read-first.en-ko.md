# #231 / #230 independent first captures · 최초 독립 검증

**Ordinary timing remains open for P and Q. 일반 P·Q 모두 시간 기준 미충족.** Candidate default-clock ordinary generation 2 exceeded 62,500 ticks on P rounds 1 (63,386) and 5 (65,215), and Q round 5 (84,605). Every other ordinary generation met its original deadline. All 40 icount3 boots met it. All 80 fresh cold boots completed the functional RAM/ELF/exception replay, including normal drains and private worker checksums. 각 셀 5회, 원본/후보 순서는 AB/BA/AB/BA/AB이며 재시도는 없다. 기본 클록의 P·Q 후보 각각에서 2세대 지연 초과를 보존했다.

Plan 0c39a79067b1a2f9cc7e8e68b14d9965ced58262. Parent 096e5a99efc0d5ef12e50e995e44e666fb99cb43. Candidate 2eac445371902a39209afa0c1318059c3cc21461. The four parent ELF hashes reproduce the public corrected-package report. The eight user ELFs share 4969269f4f9a6cf5d75e35678586f02bbccbf219129851fc73b15744aeb04e0c. One WSL ext4 build/guest at a time, CARGO_BUILD_JOBS=1. Original source and normal packaged image, matched compiler and QEMU args apart from paths and prescribed image/clock changes. WSL QEMU 결과이며 실제 보드 및 실시간 성능은 측정하지 않았다.

## Timing in original counter ticks / 원래 카운터 틱

Min/median/max across five cold runs. First opening below means generation 2 (first IRQ transition). Initial generation 1 and later generations are separated in cells.csv. Result stage is the diagnostic audit_begin minus result_begin. Ordinary packages supply no stage samples. No offset adjustment. 최초 IRQ 전환은 2세대이며 최초 디스패치는 별개다. 진단 계측 비용은 차감하지 않았다.

|Table|Clock|Software|Image|Missed boots /5|First opening→EL0|First result stage|Later result stage|
|---|---|---|---|---:|---|---|---|
|p|default|parent|plain|4|62471/70583/75314|-|-|
|p|default|parent|phase|5|62803/67680/107315|10736/10920/15714|85/338.5/1662|
|p|default|candidate|plain|2|55570/58603/65215|-|-|
|p|default|candidate|phase|3|58613/64983/68563|9313/9678/9892|138/312.0/598|
|p|icount3|parent|plain|0|1424/1424/1424|-|-|
|p|icount3|parent|phase|0|1471/1471/1471|175/175/175|175/181.5/183|
|p|icount3|candidate|plain|0|1371/1371/1371|-|-|
|p|icount3|candidate|phase|0|1417/1417/1417|122/122/122|122/128.0/130|
|q|default|parent|plain|0|47643/49663/52408|-|-|
|q|default|parent|phase|2|59381/60603/86294|10997/13603/14450|114/300/755|
|q|default|candidate|plain|1|48267/50461/84605|-|-|
|q|default|candidate|phase|1|51231/55211/67949|9308/9577/11985|85/295/1578|
|q|icount3|parent|plain|0|1429/1429/1429|-|-|
|q|icount3|parent|phase|0|1476/1476/1476|183/183/183|175/180/183|
|q|icount3|candidate|plain|0|1376/1376/1376|-|-|
|q|icount3|candidate|phase|0|1422/1422/1422|130/130/130|122/126/130|

The first diagnostic icount3 stage shrank by 53 ticks in each table. Default-clock first-stage medians dropped from 10,920 to 9,678 P and from 13,603 to 9,577 Q. Five samples per cell do not establish a host-independent latency bound. Default ordinary first-transition medians fell on P and rose on Q. Q's first transition repeats idle PID, P's changes PID. cells.csv also separates all same/cross-PID result stages. 기본 클록 단계 중앙값은 양쪽 모두 감소했으나 일반 Q의 최초 전환 지연 중앙값은 증가했다.

## Data, resource and linked code / 데이터·자원·기계어

All 920 measured windows and 160 exit observations passed provenance, private-page ownership and progress, terminal checksum×3/×5 and exit 41/42, complete 104-word restored/trapped frames, stopped frames/results, and QEMU exception order against the unchanged parent replay. RAM-origin epoch, original CVAL grid, IAR 30 and timer control were checked. analysis/result-rows.csv includes all 1,080 raw per-window/drain result records; windows.csv includes each opening, user tick, IRQ and margin derivable as 62,500 minus latency. stages.csv contains all 420 transition identities, timestamp brackets and measured first/later/same/cross-PID samples. 정상 입력 데이터와 프레임·타이머를 모든 실행에서 원본 RAM과 대조했다.

Parent ordinary P calls copy_from_user at 0x4020329c then decodes six groups of eight bytes. Candidate ordinary P helper starts at 0x40202b90. Its range and alignment checks, DAIF.I, installed flag, live TTBR0_EL1 and PID match precede the complete page iterator. Translation requires VALID, USER, NG, AF, writable mapping and PID-private physical aperture on every covered image page. After acceptance, one 64-bit load at 0x40202c7c executes six times with eight-byte stride. The linked helper then copies its 48-byte local result into the return buffer at 0x40202c9c. Source uses volatile little-endian reads each invocation. Only accepted normal workload inputs were executed. Cross-page acceptance and rejected-input behavior are source/linked-code inspection only. 후보는 IRQ와 활성 루트·PID·전 페이지 쓰기 권한을 확인하고 워드 6개를 읽는다. 임시 결과에서 반환 버퍼로 가는 memcpy는 남아 있다. 경계 및 거부 경로는 정적 검증이다.

GNU size text+rodata changes (bytes) P plain −111, phase −104; Q plain −111, phase −96. Data stays 2,720 bytes in all eight builds; ordinary BSS+stack stays 1,200,136. In both diagnostic candidates BSS+stack grows 1,632 bytes. P phase symbol (size 0x660 in both) moves from 0x4021f098 to 0x4032c008, with linked stack top moving from 0x40330010 to 0x40330670. Raw section sizes, program segments, disassembly and manifests per package are under analysis/packages. 일반 이미지의 BSS와 스택 합계는 동일하고 진단 이미지에서는 1,632바이트 증가했다.

Five fresh candidate compatibility captures (weighted share/wake × default/icount3 and startup) passed unchanged parent replays. Untouched archived ordinary P/Q, weighted and startup captures were separately replayed with the candidate tools. All nine returned their original timing/functional verdicts including historical P/default timing miss. 기존 아카이브 9개와 새 호환 대조군 5개를 따로 재생했다. Historical provenance is pinned by the original archive digest in source.txt. All capture and replay exit codes are retained separately.

[Paired first captures, controls and disassembly / 최초 대조 실행과 호환 기록 및 기계어 근거](native-partition-word-read-first.zip) · SHA-256 6ad9d7a3dc1dbb109f354f0df15c59235b0caad1bf2ca9b18d4b6ef92f52b6ab.
