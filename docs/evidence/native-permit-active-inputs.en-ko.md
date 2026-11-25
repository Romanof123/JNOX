# IPC-010 · First active-receiver cohort / 활성 수신기 최초 검증

## Result / 판정

All 40 original cold boots passed their fixed functional criteria. The 34 single-frame cases returned the intended local error and skipped protected COMMIT. The two role cases returned signed −13 for PID 2’s argument-free SVC 33 while PID 0 held a staged writer. All 36 continued to a valid sequence 4 with the output high and then an accepted sequence 5 withdrawal. Four ordinary control boots retained their expected behavior. All exit triplets were 71/72/73. The terminal virtual ENABLE register was zero in every RAM image.

독립 부팅 40회가 모두 첫 시도에서 사전에 정한 기능 기준을 충족했다. 프레임 34건은 지정한 로컬 오류로 거부되어 커널 COMMIT을 건너뛰었다. 권한 시험 2건에서는 송신 슬롯 0이 자기 쓰기 권한을 보유한 동안 슬롯 2의 인자 없는 acquire가 −13으로 거부됐다. 활성 시험 36건은 정상 순번 4 수락과 출력 ON을 확인한 후 순번 5에서 출력을 해제했다. 기존 정상·혼합 오류 대조군은 각 정책에서 한 번씩 실행했다.

The booted private revision was `3dac7eff2fa4b405d59f09bca105c56ca8b599ba`. Its public source counterpart is `85f9828f66ac766a13b73ced05b5bb90a4898104`. Both Git commit objects name the same complete tree `ef8cbc4b83bd65bc5bb47e3275d1f665404df58f`. Replay verifies the commit object bytes, reconstructs the root tree from its entire recursive manifest, and verifies the Git blob of every archived build-source file. Raw captures retain the SHA actually compiled. [Public source](https://github.com/Romanof123/JNOX/tree/85f9828f66ac766a13b73ced05b5bb90a4898104) · [Frozen criteria](https://github.com/Romanof123/JNOX/issues/397)

실제 캡처 기록은 빌드한 커밋을 가리킨다. 공개 커밋과 실행 커밋은 같은 전체 소스 트리를 공유한다. 소스 비교·검증에는 두 커밋 객체, 전체 트리 목록, 빌드에 쓰인 디렉터리의 소스 파일을 사용했다.

## Platform and measurement / 실행 조건 및 측정

Stock QEMU 6.2.0, virt GICv2, Cortex-A72, one CPU, TCG, 128 MiB, 62,500,000 Hz. FIFO and RR used the ordinary safety-channel packages, priorities 7/6/4, synchronous 2→0 and 2→1 routes, loan route 0→1, and RR quantum 625,000 ticks. Every UART was collected from a new QEMU process. The packaged manifests retain exact selectors and flags, binary hashes, and the QEMU command. All first attempts, build and capture receipts, and full terminal RAM images are included. The protected safety journal lost zero entries across the cohort.

Each decision used a sampled live counter. Sequence 1–3 and test/follow-up frame issue ticks were scheduled bench stimuli. The prior accepted issue was 275306–417398 ticks (4.404896–6.678368 ms) old at the refusal and 697099–712926 ticks (11.153584–11.406816 ms) old at the following accepted decision. The old-issue limit was strictly less than 1,250,000 ticks. Every seq4 wire issue was independently fresh at the live clock. A kernel output transition bracket and protected INFO values established virtual output state. No external device was instrumented.

발행 시각은 시험 일정에서 구성했고, 판정 시각은 실제 카운터에서 읽었다. 이전 허가가 신선한 조건은 1,250,000틱 미만으로 검사했다. 커널 INFO와 장치 변경 이벤트로 거부 전후 출력 상태를 대조했다. 스케줄러의 고정 길이 추적 버퍼에는 부팅당 277–374건의 드롭이 있었으며 보호된 안전 이벤트의 드롭은 전부 0건이었다.

The original release-to-completion and inter-completion 5 ms limits remain unchanged. All 40 of the 40 boots had at least one inter-completion interval whose conservative lower bound exceeded 312,500 ticks. The largest upper bounds were 337396 ticks for response and 355987 ticks for consecutive completions. This is a functional virtual-device result. Native timing, WCET, hardware behavior and product qualification remain open.

원래의 5 ms 시간 기준을 그대로 적용했다. 연속 COMPLETE 사이 간격은 아래 표처럼 기준을 넘었으며, 실물 하드웨어·WCET·제품 승인은 이 기능 검증의 범위 밖이다.

## First run per policy and selector / 정책·입력별 최초 부팅

Ages are integer ticks since the preceding accepted seq3 issue. SKIP is the recorded 2⁶³ sentinel. The two CRC columns are stored and independently calculated IEEE CRC-32 over bytes 0–39. G is the genuine configured generation. The packet’s per-boot JSON includes complete bytes, read and release handles, private sequence/issue/permit/recovery, protected fields, live decision samples, output writes, fixed scheduler losses, and kernel ELF and session hashes.

age는 앞서 수락한 seq3 발행 시각부터 센 틱이다. CRC는 프레임에 기록된 값과 실제 수신 바이트로 독립 계산한 값을 각각 표시한다. 프레임 거부 시 두 보호 COMPLETE에서 이전 순번·발행 시각·허가·복구 값이 유지됐고, 그 사이 COMMIT과 출력 전환도 없었다.

|Boot / 부팅|G|Local status / 로컬|COMMIT|Refusal age / 거부|Next seq4 age / 후속|Wire CRC / 계산 CRC|Fixed drops|Max COMPLETE gap lower..upper, ticks|
|---|---:|---|---|---:|---:|---|---:|---|
|01-fifo-bad-crc|1|2|SKIP|388243|697439|431ac642 / fba6a127|281|328739..328822|
|02-fifo-bad-header-crc|1|2|SKIP|388297|701243|4db4f2d3 / e2f06094|281|330545..330644|
|03-fifo-bad-length|1|1|SKIP|386461|697146|314cfdcc / 314cfdcc|280|335115..335209|
|04-fifo-bad-version|1|3|SKIP|390086|698497|b51fc895 / b51fc895|281|330280..330367|
|05-fifo-bad-type|1|3|SKIP|386238|699313|8ea574be / 8ea574be|280|331380..331504|
|06-fifo-bad-channel|1|3|SKIP|388640|703039|8bfd6c8a / 8bfd6c8a|281|331861..331957|
|07-fifo-bad-data-id|1|3|SKIP|393630|699023|b72553ba / b72553ba|280|343521..343625|
|08-fifo-bad-validity|1|3|SKIP|390044|697687|30342d3d / 30342d3d|281|333792..333879|
|09-fifo-bad-permit|1|3|SKIP|391167|703083|f95de9df / f95de9df|281|327196..327285|
|10-fifo-bad-reserved|1|3|SKIP|387226|712926|5a1864ea / 5a1864ea|281|326314..326434|
|11-fifo-duplicate|1|8|SKIP|387604|699352|1fc4dafb / 1fc4dafb|281|324614..325387|
|12-fifo-reordered|1|8|SKIP|385074|699457|fdf01f85 / fdf01f85|281|327370..327459|
|13-fifo-gap|1|8|SKIP|392208|699655|de798f0c / de798f0c|281|329389..329470|
|14-fifo-old-generation|2|7|SKIP|386936|700461|3d960061 / 3d960061|374|325244..325328|
|15-fifo-future-issue|1|9|SKIP|387290|698931|5e3561ea / 5e3561ea|281|332168..332268|
|16-fifo-expired-issue|1|10|SKIP|388688|697503|a6c190ed / a6c190ed|281|333551..333639|
|17-fifo-issue-order|1|11|SKIP|385635|700703|b0dbdd8f / b0dbdd8f|281|332661..332742|
|18-fifo-wrong-owner|1|PID 2 / −13|SVC 33|275306|700010|–|284|329591..329705|
|19-fifo-healthy|–|normal control|–|–|–|–|302|320076..320169|
|20-fifo-invalid|–|normal control|–|–|–|–|365|322682..322770|
|21-rr-bad-crc|1|2|SKIP|388973|699252|f7372d89 / 4f8b4aec|281|332331..332423|
|22-rr-bad-header-crc|1|2|SKIP|400058|702123|3a468752 / 95021515|277|339913..339991|
|23-rr-bad-length|1|1|SKIP|395095|697099|23ccc2f5 / 23ccc2f5|281|331930..332021|
|24-rr-bad-version|1|3|SKIP|388472|702443|d7dac4db / d7dac4db|281|329115..329213|
|25-rr-bad-type|1|3|SKIP|396655|711346|3ea97277 / 3ea97277|281|332014..332088|
|26-rr-bad-channel|1|3|SKIP|387052|698370|c0641d03 / c0641d03|280|328272..328353|
|27-rr-bad-data-id|1|3|SKIP|390398|700016|2ea34978 / 2ea34978|281|339157..339254|
|28-rr-bad-validity|1|3|SKIP|389025|699473|f4545829 / f4545829|281|332050..332147|
|29-rr-bad-permit|1|3|SKIP|393167|697926|111bcde5 / 111bcde5|281|330130..330207|
|30-rr-bad-reserved|1|3|SKIP|385900|701097|3c453d6c / 3c453d6c|281|329782..329880|
|31-rr-duplicate|1|8|SKIP|392036|698228|3aa12cf7 / 3aa12cf7|281|333071..333159|
|32-rr-reordered|1|8|SKIP|387251|701950|6e8f30bd / 6e8f30bd|281|326406..326517|
|33-rr-gap|1|8|SKIP|386779|697576|295b56a8 / 295b56a8|281|327741..327827|
|34-rr-old-generation|2|7|SKIP|417398|700383|2c22be14 / 2c22be14|374|355906..355987|
|35-rr-future-issue|1|9|SKIP|386096|705593|90cf44a3 / 90cf44a3|280|324131..324220|
|36-rr-expired-issue|1|10|SKIP|390312|700918|d05ca380 / d05ca380|281|328378..328469|
|37-rr-issue-order|1|11|SKIP|387065|702684|caec914c / caec914c|281|339616..339714|
|38-rr-wrong-owner|1|PID 2 / −13|SVC 33|276265|699264|–|284|330810..331383|
|39-rr-healthy|–|normal control|–|–|–|–|302|334257..334384|
|40-rr-invalid|–|normal control|–|–|–|–|365|321133..321232|

22-rr-bad-header-crc also exceeded the 5 ms release-to-COMPLETE response limit at 334879–337396 ticks (5.358064–5.398336 ms). The largest upper completion-gap bound for the cohort was 5.695792 ms. 이 부팅에서는 완료 간격뿐 아니라 릴리스부터 COMPLETE까지의 응답 시간도 5 ms를 초과했다.

## Endpoint authority and controls / 종단 권한 및 대조군

Kernel event 9 identifies the actual denied caller, operation and result. The supervisor’s own return and before/after snapshots bracket the kernel event. PID 0’s successful stage and publication rows, authenticated loan events, the protected byte snapshot and the receiver’s private copy all join to the same handle. PID 2 received synchronization words only. The early-denied SVC 33 has no post-loan type-6 event. A different PID releasing the owner’s handle (SVC 36) was outside this frozen native case.

커널 이벤트 9의 실제 호출 주체·연산·거부 코드를 감독 프로세스 전후 상태와 비교했다. 원 소유자가 단계별로 기록한 동일 핸들의 생성·게시·수신·해제를 묶었다. 다른 PID가 소유자의 핸들로 SVC 36을 호출하는 경우는 이 네이티브 시험에서 관찰하지 않았다.

|Policy|Generation|Staged/published/received handle|Supervisor bracket|Protected denial tick|PID / SVC / signed status|
|---|---:|---:|---|---:|---|
|fifo|1|16|5320472..5327521|5323907|2 / 33 / −13|
|rr|1|16|5314655..5320113|5317287|2 / 33 / −13|

FIFO healthy · Original receiver-local code sequence `[0, 0, 0, 0, 0, 0, 0]`.

FIFO invalid · Original receiver-local code sequence `[0, 0, 0, 2, 3, 9, 8, 17, 17, 17, 17, 17, 17]`.

RR healthy · Original receiver-local code sequence `[0, 0, 0, 0, 0, 0, 0]`.

RR invalid · Original receiver-local code sequence `[0, 0, 0, 2, 3, 9, 8, 17, 17, 17, 17, 17, 17]`.

Local codes are Length 1, CRC 2, Format 3, Generation 7, Sequence 8, Future 9, Expired 10, IssueOrder 11, NeedsOpen 17. Older-generation cases first genuinely operated generation 1 and then opened generation 2 at the scheduled +35 ms, with the new pair at +40/+50 ms and REARM at +55 ms. The duplicate/reordered/gap values were distinct at 3/2/5. The expired-candidate case reached its own wire-age check with the prior permit still fresh. The unchanged mixed invalid run includes post-latch NeedsOpen and has separate control status.

The offline checker rejected all 17 deliberately altered record sets. They cover bytes, CRC, transported length, local reason, COMMIT, advancement, recovery, prior faults and freshness, subsequent freshness, authorized history, future issue, absent/incorrect PID identity, original handle continuity, protected prior permit, and an intervening fault. Perturbations were in-memory copies used by replay. The original captured files retain their SHA-256 digests.

오프라인 재검증기는 기록 사본에 가한 17개 변형을 모두 탐지했다. 원본 RAM·UART에는 손대지 않았다. 정상 및 혼합 오류의 기존 결과는 위 코드열에서 확인할 수 있다.

## Offline replay / 오프라인 재검증

After unpacking the .tar.xz with GNU tar, run the following commands from the archive root. The replay uses Python 3.10 or later and GNU AArch64 binutils. Only the selected output directory is written. Compare generated JSON byte-for-byte with the supplied results.

```sh
sha256sum --quiet -c SHA256SUMS
PYTHONDONTWRITEBYTECODE=1 python3 scripts/replay.py . ../ipc397-replayed
diff -qr results ../ipc397-replayed
```

원본 해시를 확인한 뒤 위 명령으로 RAM 페이지 테이블·ELF 심볼·UART를 다시 읽는다. 40개 원본 부팅의 JSON과 변형 검증 결과를 묶음 안의 결과와 바이트 단위로 대조할 수 있다.

[Raw captures and offline replay](native-permit-active-inputs.tar.xz) (SHA-256 `510c025cab482780e188e59f0f962d0fb70d3e8da093a637ffde7632cbbe7d88`).
