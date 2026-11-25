# Corrected delivery reader / 수정된 전달 리더 독립 재검증

The unmodified public CLI at 10d20e0b8d00d14aa0810c3d3b1b149482f46007 exited 0 on the two immutable firsts. The public raw archive SHA-256 is 09f93afa85d8d32a41d5c02678e780ae68ab988ca75f0bbf04f42341bf89b7b9. Every listed member passed its digest. The original CLI exit 2 and its first result.json remain in that archive. No new guest or host calibration was run.

같은 최초 자료의 공개 아카이브와 항목 해시를 확인한 뒤 수정된 공개 CLI를 그대로 실행하여 종료 코드 0을 확인했다. 이전 CLI의 종료 코드 2와 오류 기록은 원본에 보존했다. 추가 게스트 실행과 재검교정은 없었다.

Both 15,331,376-byte final RAM captures agree with the pinned ELF layout. nm records _start at 0x40200000 (T, size 0x5c), FRAMES at 0x40212000 (d, size 0x9c0), SPACES at 0x40234000 (b, size 0xc3f000), and __stack_top at 0x4109f030 (B). Separate CLI calls against disposable copies with missing, duplicate, displaced, truncated-size and wrong-storage SPACES each rejected the capture with exit 2. An otherwise unchanged calibration receipt with the wrong boot ID withheld its trial bounds while preserving guest evidence. The actual two receipts and preflight matched platform, boot ID, UID and CPU set. The accepted child trials are distinct from vCPU measurements.

원본 ELF와 양쪽 메모리 덤프의 주소·길이가 일치했다. SPACES 누락·중복·주소 변조·크기 변조·저장 영역 변조를 각각 거부했다. 부팅 ID가 다른 대조 영수증은 실행 대기 검교정 근거에서 제외됐다.

The control scored 29 genuine refills across 89 rows, maximum 22,179 ticks. The observer scored 30 across 91 rows, maximum 16,244 ticks. Both had zero late, invalid or lost guest rows, within the 1,953,125-tick bound. Both reproduced sleep/wake rows, all nine histogram bins, reporter 3/7 and energy 5,864,240,971,776. These results agree with the earlier independent page-table and trace replay (88/90 external exceptions). QMP, raw host samples and all 120 marker callbacks join without omissions. The 1,803–23,966 ns marker widths are observational brackets. No natural miss generated a post-deadline interval to attribute. Historical misses, across-idle clock uncertainty and the lack of a demonstrated observer-cost bound persist.

대조·관측의 유효 재충전은 29·30회였고 최대 지연은 22,179·16,244틱이었다. 초과는 양쪽 모두 0회였다. 실제 앱 계산과 수면·기상 행, 기존 독립 예외 재생 결과가 일치했고 마커 120개가 대응했다. 새 초과가 없어 기한 후 지연 원인을 귀속할 수 없다. 과거 세 초과와 유휴 구간 시계 오차 및 관측 비용 상한은 미해결이다.
