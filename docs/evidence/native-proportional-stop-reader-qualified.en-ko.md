# Fixed reader offline verification / 수정 판독기 오프라인 검증

The unchanged reader at 30e6010e053157329150a99f59134d54a1b5747a accepted the original diagnostic capture from 3bdccec1401e66c9f4dcf4219abd8bf24229a58f. The original session SHA-256 was 0ca980217348561bddbe0f4738356248298604d510fb11b2c94e014a115a81b5. Every original member hash matched before and after. The first published reader's earlier symbol lookup failure remains in the original archive.

The two 848-byte EL0 stack frames at 0x40c25460 are identical. The scratch pair at +832 and +840 contains x2=0 and the entry tick 568164. Both sample PCs are 0x40203abc. The deliberately supplied caller 1 met actual owner 0, and 568164 minus 561080 gives 7084 measured ticks. A wrong session digest was rejected. Changing the saved entry by one bit on a separate offline RAM copy and resealing the copy was rejected by the new vector context check. Exact commands and results are recorded in the logs and test.py.

The fixed revision changes only reader and evidence files versus the control revision. The kernel, ordinary packager and SDK trees compare equal against f3be808. We rechecked the original and candidate ordinary PT_LOADs from the first archive for all seven configurations. This was offline replay only. No guest ran. Historical #79 remains unattributed.

공개 수정 판독기는 원본 세션을 변경 없이 재생했다. 전후의 모든 원본 해시가 일치했고 두 시점의 848바이트 프레임, 보존 x2=0, 진입 카운터 568164와 주차 PC가 일치했다. 잘못된 해시와 별도 복사본의 진입값 1비트 변경을 각각 거부했다. 일반 구성 7개의 기존 바이너리 적재 세그먼트를 다시 비교했다. 새 게스트 실행은 없었고 기존 #79의 원인은 미확인이다.
