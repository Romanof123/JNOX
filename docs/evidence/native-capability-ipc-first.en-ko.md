# Independent verification of JNOX #386 / JNOX #386 독립 검증

Frozen Git commit `912f3ec7b1d763d2ebd7e93dfddae4e084b64772`. All 15 single-core AArch64 QEMU virt boots completed. Each package manifest records an empty tracked diff. The 12 published C/Rust scenarios exited 71/72/73 under both FIFO and RR. The replay reads the actual user variables through each linked per-process root in the terminal RAM dump and checks raw UART, package hashes and process identities.

고정 커밋에서 FIFO·RR 각각 6가지 C/Rust 시나리오를 실행했다. 세 프로세스의 종료값은 모두 71/72/73이다. 각 주소 공간의 3단계 페이지 테이블을 따라 실제 수신 워드, 통지, 권한, 세대 및 호출 결과를 읽고 UART와 대조했다.

| Scenario / 사례 | Observed target samples / 대상 수신 표본 |
|---|---|
| transfer | 1, 2, 3, 4, 5, 6 |
| before | 7 |
| pending | empty / 없음 |
| active | 8 |
| target | empty / 없음 |
| mailbox | 9, 10, 11, 12, 13 |

In transfer, recipient handles 20/21/22/23/36 carried rights 1/2/1/1/3 with tags 0–4. Notice saturation returned -11 and handle saturation returned -24. The successful MOVE invalidated handle 16. The origin received handle 32 on the return transfer. The recipient peaked at four installed handles and two pending notices. All terminal owners recorded zero handles, pending notices and target references. The journals recorded zero dropped scheduler events. Payload origin is correlated from the process's recorded send operation, unique payload and target RAM. The mailbox receive ABI returns a word only. Intermediate kernel reference counts on each refused transfer were checked in deterministic host tests. The guest logs capture boot/terminal kernel counts and selected application observation points.

통지 포화에서 -11, 핸들 포화에서 -24가 반환됐고 이동 성공 후 기존 16번 핸들이 무효화됐다. 역방향 전달로 원래 소유자가 받은 핸들은 32다. 최종 커널 참조·통지·핸들은 전부 0이다. 송신자 식별은 프로세스의 호출 기록과 고유 워드 및 실제 수신 기록의 대조에 근거한다. 게스트는 시작·종료 시 커널 참조와 선택한 단계의 앱 관측값을 남겼다. 개별 실패 시 내부 상태 보존은 호스트 시험에서도 확인했다.

Two additional FIFO/RR boots exercised the actual capability send on a full mailbox. Polling returned -11. An expired absolute deadline and a future 100 ms deadline returned -110. The observed monotonic elapsed intervals were 100,585,456 ns and 100,641,920 ns, respectively for FIFO and RR. The indefinitely blocked send resumed when the receiver drained space, delivering 105 after 101–104. No refused word arrived. A separate ordinary default package exercised services 53–58, all returning ENOSYS (-38). Five host capability tests and three mailbox tests passed, including the final-generation, commit/death and expiry/promotion boundaries.

추가 게스트 2회에서 가득 찬 메일박스에 대해 폴링·지난 기한·미래 절대기한·무기한 대기를 검사했다. 타임아웃 데이터는 전달되지 않았고 무기한 송신은 공간이 생기자 순서대로 전달됐다. 기본 일반 패키지에서는 서비스 53–58이 모두 -38을 반환했다. 결정론적 호스트 시험 8개가 통과했다. 세대 고갈과 정확히 같은 시각의 사건 순서는 호스트 시험 범위다. 물리 보드 및 최악 실행시간 측정은 수행하지 않았다.

## Replay / 재검증

Extract the archive and run `sha256sum -c SHA256SUMS`, then `python3 evidence/verify.py > replay.json && cmp replay.json evidence/observations.json` on Linux with Python and GNU nm. The archive includes all captures, user and kernel ELFs, raw RAM/UART, manifests, commands, host test output, independent deadline/stock sources, and the frozen tracked source archive. For fresh boots unpack source into a Git checkout at the stated SHA, create a fresh evidence directory, copy replay.sh, extra.sh and the three independent C sources there, and run the scripts with the pinned Rust toolchain and SDK dependencies. The first two setup failures (missing Git metadata and the resulting pre-existing empty output directory) are retained in the evidence logs. No candidate changes were used.

압축을 푼 뒤 위 명령으로 해시와 관측 결과를 재검증할 수 있다. 원시 RAM/UART, 실행파일, 빌드·실행 명령, 독립 시험 소스 및 고정 원본을 포함한다. 첫 패키징에서 Git 메타데이터가 없었고 다음 시도에 기존 빈 디렉터리가 남아 발생한 오류도 보존했다.

## JNOX #386 compact evidence / 경량 증거 패키지

All 15 original guest captures, eight host-test results, ELFs, UART, RAM, command logs and observation records are preserved byte-for-byte. The only omitted member is the redundant frozen-source.tar.gz. ORIGINAL-SHA256SUMS retains its SHA and the original inventory. Original archive SHA-256 is 6bf4f230f697378be7a08e0f4b9396ef567c833d4c90418c5c50c4204dbd897f.

원본의 게스트 15회와 호스트 시험 8개 결과, ELF, UART, RAM, 명령 로그 및 관측 기록의 해시가 모두 일치한다. 중복 원본 소스 압축본 하나만 제외했으며 원래의 전체 해시 목록도 보존했다.

Frozen local revision 912f3ec7b1d763d2ebd7e93dfddae4e084b64772. Public source revision e75addbccc80e98bf0a98d975ef6dded3d450a22. Shared tree 471c00c0ca7ce1c09952b7b97585a027eff145b2.

From the extracted packet / 압축 해제 후 오프라인 재검증

    sha256sum --quiet -c SHA256SUMS
    python3 evidence/verify.py > replay.json
    cmp replay.json evidence/observations.json

The detailed bilingual report is evidence/REPORT.md. / 상세 검증 결과는 evidence/REPORT.md에 있다.

Fresh extraction passed the hash check and reproduced the observation JSON byte-for-byte. / 새로 압축을 풀어 체크섬과 관측 JSON의 바이트 일치를 확인했다.

Delivery SHA-256 `0522dd0262c485f2a8665a748fedbb8080ac8c3e56da8c7353b86fc2f5af86ef`, 2539551 bytes.

[Raw guest captures and offline replay](native-capability-ipc-first.tar.gz) (SHA-256 `0522dd0262c485f2a8665a748fedbb8080ac8c3e56da8c7353b86fc2f5af86ef`).
