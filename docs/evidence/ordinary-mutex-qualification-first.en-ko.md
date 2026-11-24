# Independent offline review / 독립 오프라인 검증

**HOLD / 수정 대기.** The two original #236 cohorts reproduce exactly. One inconsistent summary in a derived archive still receives a score. 최초 두 코호트 판정은 정확하다. 파생 자료에서 0청크와 완전한 HIGH 읽기가 동시에 기록되어도 오류 대신 점수가 출력된다.

Source 2b55db83b4269838d70583c2a6c2962a6261d540, clean detached checkout. Contract f4140f2 precedes the implementation and documents both languages, fixed position order and all three exits. Git merge-tree with main 7c84e293d4d91933cf93b55554acf8334dbf5702 produced d8265c9c982f089bb2c6a8fcb54a65447ad98937 without conflict. The CLI, legacy oracle, regression, normative plan and bilingual contract have identical Git contents in the source and combined tree. Both input ZIPs were read directly from public commit 317d7365ad966441453eabb38aeab1c78e751f7b and match the pinned SHA-256 values recorded in transcript.txt. 원본은 공개 커밋에서 추출했고, 병합 트리에서 실행 코드·판정 문서의 동일성을 확인했다.

|Position / 위치|Feature IRQ chunks / 적격 여부|Integration IRQ chunks / 적격 여부|
|---|---|---|
|PI/plain|42 PASS|57 PASS|
|PI/traced|76 PASS|0 MISS 16|
|ceiling/plain|80 PASS|18 PASS|
|ceiling/traced|29 PASS|0 MISS 16|
|Score and process exit / 점수·종료|4/4, 0|2/4, 1|

Each pass carried the in-work marker and final HIGH read of version 1, 65,536 samples, sum 557,056. Both integration misses had a cleared work marker, no progress records and an empty HIGH snapshot. For all four traced positions, independent decoding matched the recorded exception ELRs and return PCs with the UART event/restore sequences. 각 적격 실행에서 보호 작업 표시와 최종 합계를 확인했고, 통합 미달 두 건에서는 작업 표시·진행·최종 읽기가 모두 0이었다. 추적 네 건의 예외 진입·복귀 주소열을 독립 대조했다.

**Finding.** The reason-16 branch checks the IRQ and C ledger but validates only row shape/count for HIGH and the historical replay skips final summary checks on a miss. In an offline data-handling control, the PI/traced integration UART retained its original zero-work event, restore, progress and external trace. Replacing only its HIGH summary with the real feature PI/traced HIGH row and regenerating its manifest yielded exit 1 and 2/4. The terminal HIGH read cannot precede work in this capture. Expected exit 2 and no score. Recommend testing consistency of the recorded summary with progress and execution stage before admitting the capture as an observed miss. 일반 미달 경로는 HIGH 행의 형식만 확인한다. 실제 완료 원본의 HIGH 행 하나를 통합 미달 자료에 혼입한 대조군에서도 2/4를 출력했다. 진행·실행 단계와 모순되는 최종 기록은 오류 코드 2로 분리해야 한다.

Frozen fixture regression including five invalid controls passed. Independently checked a foreign AMC archive and a missing ceiling UART, both exit 2, no score. Mutex anchor tests 5/5 and graph tests 6/6 passed, full graph exit 0. No original partial-work miss exists in these two fixtures. No live launches, source edits, or external publication. 최초 자료에는 0 초과 진행 후 미달 사례가 없다. 실기·게스트 실행 없이 검토했으며 검증 레인을 해제한다.

[Original captured archive replays and first independent findings](ordinary-mutex-qualification-first.zip) · SHA-256 6edd27f3a991673d6b24830374227952a14eb615e3f9d586bb79d51240468c0e.
