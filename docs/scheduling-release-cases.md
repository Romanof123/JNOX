# 스케줄링 출시 사례별 근거와 후속 검증

[English](scheduling-release-cases.en.md) · [출시 기준](scheduling-policies.md) · [REQ-023 / VER-023 · #21](https://github.com/Romanof123/JNOX/issues/21)

기준 커밋 `19d2154`. 각 행은 기존 출시 기준의 하위 사례와 검토된 기록을 연결한다. 각 기록에 대상 커밋·설정·관찰 범위가 있다. EVD-004의 공통 선택·차감은 게스트 내부의 통제 입력이며 실제 작업 전환은 해당 작업별 기록을 따른다. EVD-008은 호스트 계산이다. 게스트는 QEMU 가상 CPU에서 실행됐다. 실물 측정과 제품 승인은 해당 프로파일의 주장에 따라 판단한다. 18개 프로파일의 출시 판정은 열려 있다.

| 정책 | 하위 사례와 기존 근거 | 남은 실행 또는 승인 |
|---|---|---|
| POL-001 | 높은 ID의 선착순, 타이머 도착, 양보·차단·깨움·종료, 처리기 내부 등록 순서, 깊이 2와 초과 거부. [EVD-004](evidence/evd-004.md) · [EVD-012](evidence/evd-012.md) 연결된 서비스 두 종류에서 큐 점유량은 각각 최대 2와 1이었다. 가득 찬 큐와 미등록 핸들러 요청을 거부했고 핸들러가 새 이벤트를 넣어도 FIFO 순서를 유지했다. 두 소유 작업 모두 핸들러 복귀 후 예산 초과를 두 번 기록하고 실행을 계속했다. [EVD-036](evidence/evd-036.md) · [#80](https://github.com/Romanof123/JNOX/issues/80) | 실기기에서 핸들러 최악 실행 시간을 검증하고 제품의 큐·핸들러·부하 한계를 승인한다. |
| POL-002 | HIGH 즉시·지연 실행, 동순위 도착·양보·종료, 상위 선점, RR 잔여량 만료, Rust 작업 및 Pi 모델. [EVD-002](evidence/evd-002.md) · [EVD-003](evidence/evd-003.md) · [EVD-004](evidence/evd-004.md) · [EVD-014](evidence/evd-014.md) · [EVD-027](evidence/evd-027.md) · [EVD-030](evidence/evd-030.md) QEMU Pi 4 모델에서는 FIFO·RR 작업의 선점과 RR 동순위 교대를 확인했다(#71). QEMU virt에서는 RR 만료와 깨움이 겹친 경우 및 IRQ 마스킹으로 깨움이 지연된 경우를 확인했다(#72). [EVD-031](evidence/evd-031.md) · [EVD-032](evidence/evd-032.md) | 실보드 검증과 승인은 #34에서 계속 추적한다. |
| POL-003 | SJF/SRTF의 4 대 2, HRRN의 10 대기한 5와 새 1, 과소 추정 작업. 순위 대조는 동점과 무효 추정을 다룬다. [EVD-004](evidence/evd-004.md) · [EVD-013](evidence/evd-013.md) | M4 제출 인터페이스에서 동점 처리와 잘못된 추정 거부를 관찰한다. |
| POL-004 | 동시·후속 주기 릴리스, 실행 중 선점, 중첩·기한 위반. EVD-008은 지터·차단·IRQ·디스패치가 있는 조건부 호스트 상한 계산. QEMU virt에서 (10,9)/(12,4) 작업 쌍은 틱 0에 RM에서 A·B, DM에서 B·A 순서로 실행됐다. 두 정책 모두 틱 10에 A, 틱 12에 B를 실행했다. [EVD-004](evidence/evd-004.md) · [EVD-007](evidence/evd-007.md) · [EVD-008](evidence/evd-008.md) · [EVD-009](evidence/evd-009.md) · [EVD-028](evidence/evd-028.md) · [EVD-033](evidence/evd-033.md) | 승인 대상 작업 집합과 검증된 비용 자료로 분석한다. |
| POL-005 | (8,1)/(10,8) 최초 선택, 이른 기한 도착, 표본 시점 교차·동점·음의 여유, PC 복귀. [EVD-004](evidence/evd-004.md) · [EVD-005](evidence/evd-005.md) · [EVD-015](evidence/evd-015.md) | 승인 작업과 간섭 조건에 재평가 간격 및 전환 빈도 상한을 적용한다. |
| POL-006 | 경계 직전·경계, 유휴·순환, 표 검사로 잘못된 슬롯 거부, 게스트 초과 실행과 다음 도메인 진입. [EVD-004](evidence/evd-004.md) · [EVD-010](evidence/evd-010.md) | 선택한 프레임·초과 조치·도메인 부하를 검증한다. |
| POL-007 | 빈 폴링, 경계 전후 도착, 서비스량·잔여 예산, 보충액·시각과 포화 병합. [EVD-004](evidence/evd-004.md) · [EVD-016](evidence/evd-016.md) | 프로파일의 요청으로 서버 한계를 확인하고 M3 제어 간섭을 측정한다. |
| POL-008 | 예약 차감·보충, CBS 고갈·유휴, 진입 거부, 경계 일치·조기·지연 깨움, 기한 갱신과 이웃 할당. [EVD-004](evidence/evd-004.md) · [EVD-006](evidence/evd-006.md) · [EVD-029](evidence/evd-029.md) | 배포 예약 설정의 승인 경로와 격리 서비스를 확인한다. |
| POL-009 | 최상위 포화, 조기 양보 누적, 강등·부스트와 경쟁 작업의 다음 실행. [EVD-004](evidence/evd-004.md) · [EVD-019](evidence/evd-019.md) | M4 응용 차단·깨움과 선택한 클래스·부스트 한계를 통합한다. |
| POL-010 | 통제 선택에서 티켓 양끝·범위 거부, 두 시드의 게스트 분포, stride 비율·차등 차감·늦은 합류·정규화. [EVD-004](evidence/evd-004.md) · [EVD-021](evidence/evd-021.md) 진단용 시험에서 의도적으로 실패시킨 첫 검사 조건을 기록했다. 일반·진단 대조군은 모두 통과했다. [EVD-035](evidence/evd-035.md) | 응용 작업에서 출시용 추첨 경로와 부하·측정 창을 검증한다. 원래 실행에서 실패한 검사 조건은 아직 밝혀지지 않았다. [#79](https://github.com/Romanof123/JNOX/issues/79) |
| POL-011 | 가중치 1·2, 실측 차감 재구성, 점유율 구간, 수면·깨움 위치와 지연. [EVD-004](evidence/evd-004.md) · [EVD-022](evidence/evd-022.md) | 통합 작업 조합에 선언한 오차·관찰 구간을 적용한다. |
| POL-012 | 가장 이른 부적격 기한, 적격·슬라이스·가중치 변경, 12회 선택, 최대 오차 25/20 단위와 깨움 응답 2개. [EVD-004](evidence/evd-004.md) · [EVD-023](evidence/evd-023.md) 고정 가중치 2/2/1과 단위 슬라이스에서 128회 할당 중 16회 깨웠다. 깨움 후 첫 실행까지 다른 작업에 최대 5회 할당했고 절대 서비스 오차는 최대 2.2퀀텀이었다. [EVD-034](evidence/evd-034.md) | 반복 깨움 중 가중치와 슬라이스를 변경하고 승인 대상 부하의 응답 및 서비스 오차 한계를 검증한다. |
| POL-013 | 동시 깨움·고정 작업·경쟁 이동, 한정 사례에서 소유권·마스크·클러스터 제한·단일 차감. [EVD-004](evidence/evd-004.md) · [EVD-017](evidence/evd-017.md) | 대상 토폴로지에서 실물 이동 비용과 공유 캐시 간섭을 측정한다. |
| POL-014 | MEDIUM 준비 중 LOW/HIGH 경합, 연쇄 기부·타임아웃·해제 복원, 즉시 상한·중첩 해제, 설정 검사. [EVD-004](evidence/evd-004.md) · [EVD-018](evidence/evd-018.md) | 상한과 중첩 깊이를 확정하고 M3 보드에서 최대 차단을 측정한다. |
| POL-015 | LO 예산 초과, HI 기한·예산, LO 제거, 재시작 거부와 정지 상태 승인 복귀, 점유 슬롯 4개. [EVD-004](evidence/evd-004.md) · [EVD-020](evidence/evd-020.md) | 제품 최대 이월 작업과 모드 전환 구간을 정하고 M5 시스템 대응을 심사한다. |
| POL-016 | 예산 있는 자손의 부모 고갈, 형제 할당·보충, 준비·빈 그룹·다단 차감을 두 구간에서 관찰. [EVD-004](evidence/evd-004.md) · [EVD-024](evidence/evd-024.md) | 설정된 그룹 수·깊이 경계에서 응용이 유발하는 중첩 깨움을 실행한다. |
| POL-017 | 모델 배치·대체 6개 사례와 가상 CPU 두 개에서 작업 루프 실제 진입 7회. [EVD-004](evidence/evd-004.md) · [EVD-025](evidence/evd-025.md) | 실물 동작점별로 같은 부하를 반복하고 열·공유 부하에서 에너지·기한 위반·깨움 지연·여유를 측정한다. |
| POL-018 | 유효·마스크 위반·오래된 제안, 연산 한도·정지, 대체·유휴·독립 예약. [EVD-004](evidence/evd-004.md) · [EVD-026](evidence/evd-026.md) | 확장 채택 시 위임 부하에서 탐지·대체 한계와 독립 작업 진행을 검증한다. |

[EVD-001](evidence/evd-001.md)은 가상 virt 부팅, [EVD-011](evidence/evd-011.md)은 Pi 모델 직렬 부팅 근거다. 두 기록은 스케줄링 전환을 판정하지 않는다.

## 다음 실행 단위

1. **POL-012 동적 설정.** 반복 깨움 도중 가중치와 슬라이스를 바꾸고, 서비스 오차와 깨움부터 첫 실행까지의 지연을 재구성해 승인 대상 부하의 명시된 한계와 대조한다. [#85](https://github.com/Romanof123/JNOX/issues/85)
2. 응용 작업의 차단과 깨움으로 중첩된 그룹 예산을 검증한다. [#84](https://github.com/Romanof123/JNOX/issues/84)
3. 유한 배치 제출 경로에서 SJF·SRTF·HRRN 디스패치를 실행한다. [#81](https://github.com/Romanof123/JNOX/issues/81)

고정 우선순위 Rust 런타임 작업 [#71](https://github.com/Romanof123/JNOX/issues/71)과 [#72](https://github.com/Romanof123/JNOX/issues/72)는 병합됐다. 이 런타임의 SVC #1은 작업을 종료한다. 주기 프로파일은 같은 번호로 작업 인스턴스 완료를, EEVDF는 차단을 요청한다.

[POL-004 M3 가상 제어 작업 및 FS-M3-01](pol-004-m3-control-plan.md#korean). CHG-044는 시뮬레이션 설계를 기록한다. 최초 런타임 검증은 부분 통과다.

POL-004 / IMP-042. v2는 부분 통과다. 최악 위상을 포함해 기본 구성 14개가 적격이었다. 기본 클록 8개는 조기 억제됐고 동일 틱 입력은 미충족이다. 진단 사례 10개는 별도 기록했다. [EVD-042](evidence/evd-042.md)

CHG-049 / IMP-050. 일반 이미지 정상 판정 0/32, 진단 모드 초기화 0/96이었다. 콜드 행렬은 가상 시간만 다룬다. [EVD-050](evidence/evd-050.md)
