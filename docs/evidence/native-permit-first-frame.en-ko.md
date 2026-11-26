# #389 — First B0 frame-processing intervals / 첫 B0 프레임 처리 구간

In 20 of the 21 first B0s that received a 44-byte frame, the longest interval in the reported partition ran from the protected receive audit to the matching release audit. This bracket includes the controller’s 44-byte private copy. In position 031, CHECK-after to snapshot audit was longest. A guest clock sample saved after the controller decoded the frame splits the subsequent release-audit → SVC60 COMMIT-entry span into two measured intervals. All 11 empty receives returned −11 and proceeded to COMPLETE. Every position from 013 through 044 has a closed join.

44바이트 프레임을 수신한 첫 B0 21건 중 20건에서 가장 긴 연속 구간은 보호된 수신 기록부터 같은 핸들의 반납 기록까지였다. 컨트롤러의 44바이트 복사가 이 구간에 포함된다. 031번에서는 CHECK 종료 표본부터 스냅샷 기록까지가 가장 길었다. 컨트롤러가 프레임 해석 뒤 저장한 게스트 시계 표본으로, 이후의 반납 기록부터 SVC60 COMMIT 진입까지를 두 구간으로 나눌 수 있었다. 빈 수신 11건은 모두 −11을 반환한 뒤 COMPLETE에 도달했다. 013–044번 중 연결에 실패한 기록은 없었다.

## Observed intervals / 관측 구간

Each cell gives median [minimum–maximum] in guest CNTPCT ticks. The clock was 62.5 MHz, or 62,500 ticks per millisecond. Counts apply to each column. Each total is calculated for an individual boot from its two named samples before grouping. CSVs retain every observed interval and the FIFO/RR and healthy/overrun labels.

단위는 게스트 CNTPCT 틱이며 표기는 중앙값 [최솟값–최댓값]이다. 62,500틱이 1밀리초다. 전체 구간 값은 각 부팅의 시작·끝 표본 차이를 먼저 구한 뒤 집계했다. CSV에는 각 부팅의 정책과 시나리오도 들어 있다.

| B0 population / 분류 | Count / 건수 | Stage 2 entry → stage 3 entry | CHECK-after → COMPLETE entry |
|---|---:|---:|---:|
| c5e6a374a · 44 bytes | 6 | 68,940 [65,819–77,983] | 63,797.5 [61,285–72,264] |
| c5e6a374a · empty | 10 | 8,290.5 [6,375–16,952] | 4,505 [3,888–9,406] |
| d2bd39091 · 44 bytes | 15 | 69,718 [67,285–97,251] | 64,734 [62,404–87,882] |
| d2bd39091 · empty | 1 | 7,826 [7,826–7,826] | 4,284 [4,284–4,284] |

| Frame-bearing B0 interval / 유효 프레임 구간 | c5e6a374a · n=6 | d2bd39091 · n=15 |
|---|---:|---:|
| CHECK-after → frame snapshot audit | 9,759.5 [9,578–14,293] | 11,128 [10,741–23,066] |
| Snapshot audit → receive audit | 801.5 [791–1,070] | 815 [778–1,049] |
| Receive audit → release audit | 16,533 [16,209–20,825] | 16,574 [16,255–22,735] |
| Release audit → post-decode INFO sample | 13,804 [12,698–16,245] | 12,815 [12,012–15,611] |
| Post-decode INFO sample → COMMIT entry | 8,226.5 [7,892–8,396] | 8,007 [7,754–8,761] |
| Whole release audit → COMMIT entry | 22,060.5 [20,790–24,641] | 20,893 [19,920–24,372] |
| COMMIT entry → precheck operand (site 2) | 470 [460–578] | 463 [453–486] |
| Precheck → post-CRC operand (0x102) | 3,544 [3,508–3,731] | 4,655 [4,222–5,066] |
| Post-CRC → field recheck operand (0x202) | 4,328 [3,979–5,253] | 3,591 [3,499–5,544] |
| Field recheck → accepted COMMIT status | 2,680.5 [2,358–3,941] | 2,313 [2,253–2,791] |
| COMMIT status → QUAL-after | 40.5 [37–64] | 350 [343–705] |
| Whole COMMIT entry → QUAL-after | 11,317.5 [10,676–12,398] | 11,319 [10,899–13,824] |
| COMMIT-after → COMPLETE entry | 2,945 [2,799–3,480] | 2,939 [2,705–4,823] |

The contiguous receive-audit → COMMIT-entry span accounted for a median 60.7% [58.7–62.9%] of CHECK-after → COMPLETE-entry on c5e6a374a and 58.4% [53.6–59.8%] on d2bd39091. Shares use each boot’s own numerator and denominator. The source alternated refs across a fixed set of QEMU boots. The frame-bearing populations differ, and the candidate has one empty observation. These observed durations support neither causal attribution nor a worst-case execution-time bound.

수신 기록부터 COMMIT 진입까지가 CHECK 종료 표본부터 COMPLETE 진입까지에서 차지한 비중의 중앙값은 c5e6a374a에서 60.7% [58.7–62.9%], d2bd39091에서 58.4% [53.6–59.8%]였다. 각 부팅의 분자와 분모를 짝지어 계산했다. 두 리비전의 유효 수신 건수가 다르고 후보 리비전의 빈 수신 관측은 한 건이다. 고정된 QEMU 부팅에서 측정한 이 값으로 변경의 인과 효과나 최악 실행시간 상한을 정할 수 없다.

## Reading the samples / 표본의 의미

- QUAL kind 1 locates controller PID 1, job 0, phase 2 (B), stages 2 and 3. Their SVC62 entry samples bound the measured span. Release target, runnable/dispatch and blocking timestamps fall outside this definition. QUAL kind 4 provides the synchronous entry, pre-service counter and post-service counter for CHECK (63/1), COMMIT (60/2) and COMPLETE (63/3). Its after sample precedes final dispatch and architectural return. The folded phase’s full response-time measurements are in the source comparison. [Source report](https://github.com/Romanof123/JNOX/blob/915a7eac550a1a12fd74b46eab9a3a7dbc9ca70d/docs/evidence/native-permit-service-comparison.en-ko.md)
- SAFETY event 7 captures the 44-byte wire image. The decoder checks its wire fields and CRC, generation, sequence and issue. Event 6 records receive (35) and matching release (36). Their append timestamps enclose checks on the loan, the user’s private copy, and portions of kernel handling. An empty result has no returned frame, release, commit or site samples. The CSV leaves those fields blank. The occupancy label describes the receive result. The journal’s separate count of frames remaining after the attempt was zero for both paths.
- The B0 controller RAM row at `admission_controller_rows[1][6]` holds the INFO clock sample obtained by `Receiver::accept` immediately after its private decode. Release audit → INFO sample includes release return, private decoding/CRC/format checks and INFO entry. INFO sample → COMMIT entry includes INFO return, state/freshness/sequence validation, staging and entry to the protected call. The replay matches the row’s target, fused endpoint samples, three receive/accept/commit statuses and the private and protected final sequence/issue. Event 7 and both event 6 records establish the handle. Event 7, the event 12 sites and event 5 establish generation. The retained FIXED prefix supplies a PID-1 SVC61 pre-service sample between release audit and INFO readback on every valid frame. Later FIXED loss does not enter the elapsed-time calculation.
- SAFETY event 12 contains the counter **operand** sampled for the check and a later journal append timestamp. Site 2 → 0x102 includes the first check and journal, commit guards, snapshot decoding and the kernel CRC computation. A CRC-only start/end pair is unavailable. Site 0x102 → 0x202 includes the old-state recheck and journal, CRC-result comparison, field and caller-claim acceptance. Site 0x202 → event 5 status covers the last freshness check, session updates and logging. All three sites must carry the old accepted state in source order. Event 5 captures the new generation, sequence and issue, and event 10 confirms the completed state.
- CSV columns starting with `dt_` partition the observed path. Adding them within a row reproduces `total_stage2_to_stage3`. Columns starting with `total_` span multiple samples and may overlap those intervals. All elapsed values are raw guest-counter differences and include any instrumentation or scheduling between endpoints.

QUAL은 B0 마커와 서비스 경계를, SAFETY는 같은 게스트 시계로 보호 상태와 프레임을 기록한다. 단계 2→3은 두 SVC62의 진입 표본 사이이고, 서비스의 QUAL-after는 최종 디스패치와 사용자 모드 복귀에 앞선다. RAM에 저장된 INFO 표본 전에는 반납 복귀, 복사본의 CRC·형식 검사와 INFO 진입이 있고, 이후에는 INFO 복귀, 상태·유효 기간·순번 검사와 COMMIT 진입이 있다. 이벤트 12의 검사 피연산자와 이벤트를 기록한 시각을 별개로 보존했다. 사이트 2→0x102에는 첫 검사·기록, 스냅샷 해석과 커널 CRC 계산이 함께 들어간다. 0x102→0x202에는 기존 상태 재검사 및 필드 대조가, 0x202→성공 상태 기록에는 최종 시간 검사와 상태 반영이 들어간다. 빈 수신 행의 프레임 전용 칸은 비어 있다. 각 행에서 `dt_` 열의 합은 단계 2→3 실측 총계와 같다.

## Offline replay / 오프라인 재현

The 32 session hashes in `input-sessions.csv` pin the original capture receipts. Replay verifies each UART and launch manifest against its session, checks the boot ref and zero loss in both protected journals, and independently reconstructs each join before comparing identity, service and seven pairs of interval endpoints against the sealed source CSV. It reads the additional guest clock from the hash-pinned `results/boots.json` RAM reduction. Eighteen in-memory controls exercise corrupt or wrong UARTs, truncated/partial records, frame/handle/state mismatches, duplicated/missing/misordered events, invalid QUAL/RAM samples and disagreement with the source CSV. Only the receipt-layer tests invoke hash authentication on altered bytes. Semantic tests call the decoder directly. Original captures are read-only.

세션 영수증 해시 32개를 고정하고, 각 UART·실행 명세의 해시와 두 보호 저널의 무손실 기록을 검증한다. 식별자와 서비스 상태 외에도 구간 일곱 쌍을 원본 비교 CSV와 대조했다. 추가 시계 표본은 해시를 검증한 RAM 판독본에서 읽었다. 메모리 사본을 이용한 음성 대조 18건은 각각 예상된 오류로 거부됐다. 원본 UART는 수정하지 않는다.

Use Python 3.9+ with its standard library. Extract the interval packet and run these commands from the directory containing `frame_intervals.py`. Pass the source archive below as the first argument. The compact public archive contains all required receipts as ordinary members. Its nested ELF/RAM archive can stay compressed. An extracted/restored source directory or the original full archive works with the same command. [Source archive](https://github.com/Romanof123/JNOX/blob/915a7eac550a1a12fd74b46eab9a3a7dbc9ca70d/docs/evidence/native-permit-service-comparison.tar.gz)

분석 묶음을 압축 해제한 폴더에서 아래 명령을 실행한다. 첫 인수에는 원본 비교 묶음의 경로를 지정한다. 공개 축소본에서는 ELF/RAM 파일을 복원하지 않아도 이 분석을 직접 재현할 수 있다.

~~~sh
python3 -B frame_intervals.py /path/to/native-permit-service-comparison.tar.gz replay-out
diff -r evidence replay-out
~~~

Expected output is `32 closed positions, 0 open, 21 valid44, 11 empty, 18 expected refusals`. A successful comparison prints no diff. An open join records the affected position and reason, exits nonzero and leaves the aggregate unpublished. Verified replays from the original live capture directory, full sealed archive and compact archive produced identical CSV bytes.

The full RAM verification independently reran the source packet’s authenticated receipt and ELF/page-table decoder. It matched all 18 B0 controller words in 32/32 boots. To reproduce it, restore the full source packet including `binary-images.tar.xz` if using the compact version, install GNU `aarch64-linux-gnu-nm` and run `python3 -B verify_controller_ram.py /path/to/restored/source`. 전체 RAM 재검증에서는 원본 패킷의 ELF·페이지 테이블 판독기로 32개 부팅의 B0 컨트롤러 기록을 다시 읽었고, 부팅별 18개 값이 모두 일치했다.

| Source / 원본 | SHA-256 |
|---|---|
| Public compact / 공개 축소본 | `a9c4ed9bc4ffcb1fec160b6cf5ef7cd5c75bd67302f84dee0f4220cf1cf112b7` |
| Original full / 최초 전체본 | `6c748003c73cbad84cf23b7708475d668b3ce3518b82914f07a4a3dc379b7472` |

[Raw captures and offline replay](native-permit-first-frame.tar.gz) (SHA-256 `fb76abf2f5671493195e0f8d723b02191f0023ce7da138e3493eac7879bffe4c`).
