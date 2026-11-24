# 스케줄링 출시 사례별 근거와 후속 검증

[English](scheduling-release-cases.en.md) · [출시 기준](scheduling-policies.md) · [REQ-023 / VER-023 · #21](https://github.com/Romanof123/JNOX/issues/21)

기준 커밋 `19d2154`. 각 행은 기존 출시 기준의 하위 사례와 검토된 기록을 연결한다. 각 기록에 대상 커밋·설정·관찰 범위가 있다. EVD-004의 공통 선택·차감은 게스트 내부의 통제 입력이며 실제 작업 전환은 해당 작업별 기록을 따른다. EVD-008은 호스트 계산이다. 게스트는 QEMU 가상 CPU에서 실행됐다. 실물 측정과 제품 승인은 해당 프로파일의 주장에 따라 판단한다. 18개 프로파일의 출시 판정은 열려 있다.

| 정책 | 하위 사례와 기존 근거 | 남은 실행 또는 승인 |
|---|---|---|
| POL-001 | 높은 ID의 선착순, 타이머 도착, 양보·차단·깨움·종료, 처리기 내부 등록 순서, 깊이 2와 초과 거부. [EVD-004](evidence/evd-004.md) · [EVD-012](evidence/evd-012.md) 연결된 서비스 두 종류에서 큐 점유량은 각각 최대 2와 1이었다. 가득 찬 큐와 미등록 핸들러 요청을 거부했고 핸들러가 새 이벤트를 넣어도 FIFO 순서를 유지했다. 두 소유 작업 모두 핸들러 복귀 후 예산 초과를 두 번 기록하고 실행을 계속했다. [EVD-036](evidence/evd-036.md) · [#80](https://github.com/Romanof123/JNOX/issues/80) | 실기기에서 핸들러 최악 실행 시간을 검증하고 제품의 큐·핸들러·부하 한계를 승인한다. |
| POL-002 | HIGH 즉시·지연 실행, 동순위 도착·양보·종료, 상위 선점, RR 잔여량 만료, Rust 작업 및 Pi 모델. [EVD-002](evidence/evd-002.md) · [EVD-003](evidence/evd-003.md) · [EVD-004](evidence/evd-004.md) · [EVD-014](evidence/evd-014.md) · [EVD-027](evidence/evd-027.md) · [EVD-030](evidence/evd-030.md) QEMU Pi 4 모델에서는 FIFO·RR 작업의 선점과 RR 동순위 교대를 확인했다(#71). QEMU virt에서는 RR 만료와 깨움이 겹친 경우 및 IRQ 마스킹으로 깨움이 지연된 경우를 확인했다(#72). [EVD-031](evidence/evd-031.md) · [EVD-032](evidence/evd-032.md) QEMU virt에서 FIFO·RR 구성의 콜드 부팅 24회로 관측된 핸들러·정책 실행 구간과 작업 스택 사용량을 기록했다. [EVD-041](evidence/evd-041.md) #142는 일반 UART 호스트 수집·재생을 검증했다. #146 실험 브랜치의 첫 FIFO/RR 모델 캡처는 디코더가 둘 다 거부했고 수정 후 같은 바이트가 모두 통과했다(추가 부팅 0). [#142](evidence/pi4-fixed-serial-host.md) · [최초 거부](https://github.com/Romanof123/JNOX/blob/8671e910ea67bd0433b817a537aa7eb2e8f3bf97/docs/evidence/pi4-fixed-witness-first.md) · [재생](https://github.com/Romanof123/JNOX/blob/8671e910ea67bd0433b817a537aa7eb2e8f3bf97/docs/evidence/pi4-fixed-witness-replay.md) 새 모델 직접 로드 4건에서 일반 FIFO·RR의 UART 순서를 재현했다. 엔지니어링 FIFO는 완료(코드 0), RR은 동료 복귀 두 건의 후속 작업 진행 표본이 없어 부분 관측(코드 2)이었다. 일반 .text는 main과 일치했으나 각 raw 이미지의 소스 행 번호 2바이트가 달랐다. 펌웨어 해시는 확인했고 실물 부팅은 미검증이다. [#146](https://github.com/Romanof123/JNOX/blob/6d6192749baac6d0439e162fb7edae8c85bcb17d/docs/evidence/pi4-fixed-witness-integration.md) | 실보드 검증과 승인은 #34에서 계속 추적한다. |
| POL-003 | EVD-013에서 명시한 4 대 2 도착과 HRRN 대기 시간 예제를 확인했다. 4슬롯 M4 응용의 일반 부팅 6회와 진단 부팅 3회에서 동순위 순서·잘못된 추정치·추정 초과 실행을 확인했다. 독립 검증자는 결정 60회를 재구성해 측정 서비스량, SJF·HRRN의 실행 작업 유지, SRTF의 중단 프레임·복귀 PC 복원을 대조했다. [EVD-004](evidence/evd-004.md) · [EVD-013](evidence/evd-013.md) · [EVD-037](evidence/evd-037.md) | 일반 M4 작업 부하에서 명시된 기능 사례를 확인했다. 배포 검토에는 승인 대상 작업 구성과 용량 근거가 필요하다. 최악 실행 시간을 보장하려면 해당 부하의 측정 근거가 필요하다. |
| POL-004 | 동시·후속 주기 릴리스, 실행 중 선점, 중첩·기한 위반. EVD-008은 지터·차단·IRQ·디스패치가 있는 조건부 호스트 상한 계산. QEMU virt에서 (10,9)/(12,4) 작업 쌍은 틱 0에 RM에서 A·B, DM에서 B·A 순서로 실행됐다. 두 정책 모두 틱 10에 A, 틱 12에 B를 실행했다. [EVD-004](evidence/evd-004.md) · [EVD-007](evidence/evd-007.md) · [EVD-008](evidence/evd-008.md) · [EVD-009](evidence/evd-009.md) · [EVD-028](evidence/evd-028.md) · [EVD-033](evidence/evd-033.md) 독립 호스트 재현에서 명시된 입력에 대한 위상별 수용 산술과 CLI 동작을 검증했다. [EVD-043](evidence/evd-043.md) v3 주 시험은 첫 시도에서 23건 중 14건이 적격이었고 호스트 읽기 오류 후 콜드 재시도 1회를 포함하면 15건이었다. 결정론적 최악 위상과 동일 틱 사례는 적격이었고 기본 클록 일반 사례 8건은 모두 부적격이었다. 진단 10건은 별도 판정했다. [EVD-042](evidence/evd-042.md) 콜드 24회 중 22회가 억제됐고 기술자 DM 2회는 허가 7건을 모두 수락한 뒤에도 OFF였다. 정상 시퀀스 0/24. [EVD-051](evidence/evd-051.md)  실제 저널 4개의 104개 위상 재생 일치. 통합 제어 실행의 운영 적격 0/6. [EVD-052](evidence/evd-052.md) 운영 0/20. 결정적 OFF 4건에서 허가 28건을 수락했고 출력은 OFF였다. [EVD-054](evidence/evd-054.md) 별도 통합 32자리 코호트도 운영 0/20이었다. A0 세 건은 허가 7개씩 수락했으나 첫 발행 간격이 짧았다. 결정적 OFF 4건은 각각 7개를 전달하고 OFF를 유지했다. 최종 비교는 21개 로더 입력 일치와 44개 선택기 거부를 확인했다(신규 부팅 0). [통합 코호트](evidence/evd-054-integration.md) · [호환성](evidence/evd-054-targeted.md) #144 기본 클록 상시 OFF 모델 연구는 첫 실행 16건(8쌍)을 모두 완료했다. 허가 발행과 출력 활성화는 0건이었다. R−F 시간 차이의 방향은 일정하지 않았고 인과적 속도 향상과 실물 타이밍은 미확인이다. [#144](https://github.com/Romanof123/JNOX/blob/2c79c5f0aaf1bc759fdb21c4a8d6dd8bbb0f102f/docs/evidence/permit-observer-cost-v3.md) | 승인 대상 작업 집합과 검증된 비용 자료로 분석한다.  [#109](control-cold-plan.md) |
| POL-005 | (8,1)/(10,8) 최초 선택, 이른 기한 도착, 표본 시점 교차·동점·음의 여유, PC 복귀. [EVD-004](evidence/evd-004.md) · [EVD-005](evidence/evd-005.md) · [EVD-015](evidence/evd-015.md) 일반 작업 8개 이미지의 선점·복귀, 소진 뒤 서비스와 기한 내 완료. [EVD-048](evidence/evd-048.md) | 승인 작업과 간섭 조건에 재평가 간격 및 전환 빈도 상한을 적용한다. |
| POL-006 | 경계 직전·경계, 유휴·순환, 표 검사로 잘못된 슬롯 거부, 게스트 초과 실행과 다음 도메인 진입. [EVD-004](evidence/evd-004.md) · [EVD-010](evidence/evd-010.md) / [EVD-044](evidence/evd-044.md) [EVD-047](evidence/evd-047.md) | 선택한 프레임·초과 조치·도메인 부하를 검증한다. |
| POL-007 | 빈 폴링, 경계 전후 도착, 서비스량·잔여 예산, 보충액·시각과 포화 병합. [EVD-004](evidence/evd-004.md) · [EVD-016](evidence/evd-016.md) 단독 기본 클록은 10/10건이 중재 전에 정지했다. 같은 ELF의 통합 기본 클록과 명령 계수에서는 각각 정상 8/8건과 지연 대조 2/2건을 확인했다. 타이밍 차이 원인은 미확정이다. [EVD-045](evidence/evd-045.md) | 기본 클록 시간 적격성과 예산 근거를 확보하고 실제 M3 간섭 및 실물 WCET를 검증한다. |
| POL-008 | 예약 차감·보충, CBS 고갈·유휴, 진입 거부, 경계 일치·조기·지연 깨움, 기한 갱신과 이웃 할당. [EVD-004](evidence/evd-004.md) · [EVD-006](evidence/evd-006.md) · [EVD-029](evidence/evd-029.md) 실험 브랜치 EVD-053의 일반 실행 완료는 v2 1/8, v3 4/8(같은 비교의 v2 5/8)이었다. 별도 컴파일러 비교는 크기 최적화 1/8, O2 0/8이며 양쪽 모두 과부하 두 건을 거부했다. 8/8 기준과 1 ms 한계는 미충족 상태로 유지한다. [v2](https://github.com/Romanof123/JNOX/blob/fc04768928fced018ed453de893c43aecb6a9363/docs/evidence/evd-053-v2.md) · [v3](https://github.com/Romanof123/JNOX/blob/fc04768928fced018ed453de893c43aecb6a9363/docs/evidence/evd-053-v3.md) · [compiler](https://github.com/Romanof123/JNOX/blob/fc04768928fced018ed453de893c43aecb6a9363/docs/evidence/evd-053-compiler.md) | 배포 예약 설정의 승인 경로와 격리 서비스를 확인한다. |
| POL-009 | 최상위 포화, 조기 양보 누적, 강등·부스트와 경쟁 작업의 다음 실행. [EVD-004](evidence/evd-004.md) · [EVD-019](evidence/evd-019.md) 일반 MLQ·MLFQ와 각각의 진단 구성을 콜드 부팅했다. MLQ 결정 15회에서 상위 클래스 두 작업이 교대했고 하위 작업은 둘 다 종료한 뒤 실행됐다. MLFQ 결정 42회에서 작업은 차단·깨움 후 남은 할당량을 유지했고 누적 실행량에 따라 강등됐으며 부스트 후 재개됐다. [EVD-040](evidence/evd-040.md) · [#89](https://github.com/Romanof123/JNOX/issues/89) 유한 작업 6종의 일반·진단 이미지가 첫 콜드 부팅 12회를 모두 통과했다. 이 실행에서 전체 차단·종료, 유한 포화와 세 부스트 주기의 단일 재설정을 확인했다. [EVD-046](evidence/evd-046.md) | 실제 적용할 클래스·부스트 설정과 부하·지연 한계를 검증한다. 유한 MLQ 실행으로는 지속 포화 시 하위 클래스 최소 지분을 확정할 수 없다. |
| POL-010 | 통제 선택에서 티켓 양끝·범위 거부, 두 시드의 게스트 분포, stride 비율·차등 차감·늦은 합류·정규화. [EVD-004](evidence/evd-004.md) · [EVD-021](evidence/evd-021.md) 진단용 시험에서 의도적으로 실패시킨 첫 검사 조건을 기록했다. 일반·진단 대조군은 모두 통과했다. [EVD-035](evidence/evd-035.md)  일반 Rust 최초 부팅 3회에서 두 시드와 C 합류를 재생했다. 두 진단 사례는 미실행이다. [EVD-055](evidence/evd-055.md) 별도 통합 부팅 3회에서 시드별 A/B/C 부여 횟수는 10/8/14 및 5/11/16, 스트라이드의 실측 정수 서비스 틱은 6/12/8이었다. lottery-miss와 stride-delay의 독립 실행은 미결이다. [통합 코호트](evidence/evd-055-integration.md) 복권 진단은 의도한 한 틱 조건 거부(실측 2, 기대 1)를 기록했다. 스트라이드 진단은 A에 두 단위를 부과했고 세 단위 기준은 미충족이다. [최초 진단 기록](evidence/evd-055-diagnostics.md) | 응용 작업에서 출시용 추첨 경로와 부하·측정 창을 검증한다. 원래 실행에서 실패한 검사 조건은 아직 밝혀지지 않았다. [#79](https://github.com/Romanof123/JNOX/issues/79) |
| POL-011 | 가중치 1·2, 실측 차감 재구성, 점유율 구간, 수면·깨움 위치와 지연. [EVD-004](evidence/evd-004.md) · [EVD-022](evidence/evd-022.md)  8회 콜드 실행의 1:2 서비스와 12회 동료 깨움. [EVD-049](evidence/evd-049.md) | 통합 작업 조합에 선언한 오차·관찰 구간을 적용한다. |
| POL-012 | 가장 이른 부적격 기한, 적격·슬라이스·가중치 변경, 12회 선택, 최대 오차 25/20 단위와 깨움 응답 2개. [EVD-004](evidence/evd-004.md) · [EVD-023](evidence/evd-023.md) 고정 가중치 2/2/1과 단위 슬라이스에서 128회 할당 중 16회 깨웠다. 깨움 후 첫 실행까지 다른 작업에 최대 5회 할당했고 절대 서비스 오차는 최대 2.2퀀텀이었다. [EVD-034](evidence/evd-034.md) 256회 할당과 20회 깨움 중 설정 변경 2건이 적용됐고 무효 요청 2건은 상태를 보존했다. 최대 절대 서비스 오차는 266/60퀀텀(한계 20), 깨움 후 최대 중간 할당은 6회(한계 8), 작업 코드까지 최대 지연은 4,599,989카운터 틱(한계 6,250,000)이었다. [EVD-039](evidence/evd-039.md) 텔레메트리 최초 부팅에서 서비스 창 5/5개와 주기 응답 8/8건을 충족했다. 예정→작업 최대 817,052틱(한계 2,500,000). [최초 실행](evidence/ordinary-eevdf-first.md) | 승인 대상 응용 작업 구성·측정 구간·실행 환경에서 서비스 오차와 깨움 응답 한계를 검증한다. [일반 Rust EEVDF 작업 및 실서비스 측정 구간](ordinary-eevdf-plan.md) |
| POL-013 | 동시 깨움·고정 작업·경쟁 이동, 한정 사례에서 소유권·마스크·클러스터 제한·단일 차감. [EVD-004](evidence/evd-004.md) · [EVD-017](evidence/evd-017.md) 콜드 부팅 2회에서 각각 CPU0→CPU1 지정 SGI 2회, 수신 작업 완료, 선점된 고정 백그라운드 작업 복귀를 기록했다. [EVD-056](evidence/evd-056.md) | 대상 토폴로지에서 실물 이동 비용과 공유 캐시 간섭을 측정한다. |
| POL-014 | MEDIUM 준비 중 LOW/HIGH 경합, 연쇄 기부·타임아웃·해제 복원, 즉시 상한·중첩 해제, 설정 검사. [EVD-004](evidence/evd-004.md) · [EVD-018](evidence/evd-018.md) #148에 일반 Rust 작업의 보호 데이터 인계 계획을 기록했다. #149 실험 소스는 우선순위 상속 및 천장 작업을 제공한다. [#148](ordinary-mutex-plan.md) · [#149](https://github.com/Romanof123/JNOX/blob/47ea89049b3efd2dd74642562a9973e15ad26a15/docs/ordinary-mutex-runtime.md) 최초 콜드 부팅 4건 중 3건이 작업을 충족했다. PI 일반은 보호 계산 전에 사유 16으로 중단돼 청크가 0개였다. PI 추적과 천장 일반·추적은 각각 256개를 완료했다. PI의 두 실행 충족 기준은 미달이다. [최초 실행](https://github.com/Romanof123/JNOX/blob/29a95795b780e8092aa2633b5ecde4730cf8926f/docs/evidence/ordinary-mutex-first/README.md) 선택형 앵커별 결과는 3/4, 3/4, 4/4다. 통합 첫 부팅 결과는 각각 3/4, 2/4, 4/4다. [EVD-057](evidence/evd-057.md) | 상한과 중첩 깊이를 확정하고 M3 보드에서 최대 차단을 측정한다. |
| POL-015 | LO 예산 초과, HI 기한·예산, LO 제거, 재시작 거부와 정지 상태 승인 복귀, 점유 슬롯 4개. [EVD-004](evidence/evd-004.md) · [EVD-020](evidence/evd-020.md) QEMU virt 최초 부팅은 LO 일치 사례만 통과했다(1/3). 두 전이 사례는 b6에서 새 작업의 결과를 관측하지 못했다. 수정 후 세 사례 모두 통과했고(3/3), 두 전이에서 b6 전에 유한 작업이 완료됐다. [#165 v1](evidence/ordinary-amc-first.md) / [EVD-058](evidence/ordinary-amc-finite-first.en-ko.md) | [Rust 작업 설계](ordinary-amc-plan.md) 제품 최대 이월 작업과 모드 전환 구간을 정하고 M5 시스템 대응을 심사한다. |
| POL-016 | 예산 있는 자손의 부모 고갈, 형제 할당·보충, 준비·빈 그룹·다단 차감을 두 구간에서 관찰. [EVD-004](evidence/evd-004.md) · [EVD-024](evidence/evd-024.md) 네 번의 부팅에서 작업이 요청한 차단과 깨움을 확인했다. 지연 진단 실행에서 18,907,440틱 구간을 실행 중인 리프·하위 그룹·상위 그룹 각각에 한 번씩 부과했다. 다른 분기의 작업은 자신이 실행한 143,777틱만 부담했다. 상위 그룹은 11,437,940틱의 부채를 세 차례 추가 보충 후 회복했고 유휴 구간 부과량은 0이었다. [EVD-038](evidence/evd-038.md) | 배포할 계층의 깊이·부하에 맞춰 반복 지연과 산술 한계를 검증하고 비용·격리 한계를 확정한다. |
| POL-017 | 모델 배치·대체 6개 사례와 가상 CPU 두 개에서 작업 루프 실제 진입 7회. [EVD-004](evidence/evd-004.md) · [EVD-025](evidence/evd-025.md) | 실물 동작점별로 같은 부하를 반복하고 열·공유 부하에서 에너지·기한 위반·깨움 지연·여유를 측정한다. |
| POL-018 | 유효·마스크 위반·오래된 제안, 연산 한도·정지, 대체·유휴·독립 예약. [EVD-004](evidence/evd-004.md) · [EVD-026](evidence/evd-026.md) | 확장 채택 시 위임 부하에서 탐지·대체 한계와 독립 작업 진행을 검증한다. |

[EVD-001](evidence/evd-001.md)은 가상 virt 부팅, [EVD-011](evidence/evd-011.md)은 Pi 모델 직렬 부팅 근거다. 두 기록은 스케줄링 전환을 판정하지 않는다.

## 다음 실행 단위

1. 계획된 가상 M3 제어 작업을 실행하고 시간을 독립 검증한다. [FS-M3-01](pol-004-m3-control-plan.md#korean)

고정 우선순위 Rust 런타임 작업 [#71](https://github.com/Romanof123/JNOX/issues/71)과 [#72](https://github.com/Romanof123/JNOX/issues/72)는 병합됐다. 이 런타임의 SVC #1은 작업을 종료한다. 주기 프로파일은 같은 번호로 작업 인스턴스 완료를, EEVDF는 차단을 요청한다.

## POL-003 / #81 일반 작업 승인 기준

virt에서 Cargo 벤치 기능을 끄고 JNOX_PROFILE=batch와 JNOX_APP=batch-sjf, batch-srtf 또는 batch-hrrn을 선택한다. 세 구성 모두 JNOX_BATCH_TRACE=1 및 추적 해제 상태로 실행한다. 생산자가 같은 크기의 선행 작업 두 개와 카운터 기준 0.1초 뒤 도착할 짧은 작업을 제출한다. 작업 시작·종료, 다른 작업 진입 및 체크섬 보존을 비교한다. 정한 구간에서 SJF는 생산자 종료 후 짧은 작업, SRTF는 생산자 실행 도중 짧은 작업, HRRN은 선행 작업 두 개를 순서대로 실행한 뒤 짧은 작업을 선택한다. 잘못된 추정치, 도착 시각 오버플로, 타 작업·만료 핸들의 완료 및 용량 초과를 거절해야 한다. 한 틱 추정치를 초과한 작업이 한 틱 작업을 제출하면 실행권을 유지해야 한다. 전체 작업의 종료를 확인한다.

[배치 작업 안내서](ordinary-batch.md)에 진단 기록의 15개 필드, 빌드 선택자 및 QMP 재검증 명령을 정의했다. SRTF에서 타이머가 생산자 코드를 중단하고 짧은 작업 완료 시 같은 주소와 ELR을 복원하는지 확인한다. SJF·HRRN의 실행 작업 유지도 확인한다. 추적은 최대 64건이며 마지막 완료 사건을 포함하며 전체 작업 종료 후 실제 유휴 루프에서 출력한다. 잘린 기록은 승인하지 않는다. 실제 서비스 관측값으로 순위를 결정하므로 예상 밖 순서는 시간과 대조하고 실패 기록도 보존한다.

POL-016 · CHG-040 · IMP-038 · [일반 중첩 그룹](ordinary-groups.md#한국어) · [부채와 회복 EVD-038](evidence/evd-038.md)

[POL-012 실행 중 설정 변경](m4-eevdf-dynamic-design.md) · CHG-041 / IMP-039 · [EVD-039](evidence/evd-039.md)

[POL-004 M3 가상 제어 작업 및 FS-M3-01](pol-004-m3-control-plan.md#korean). CHG-044는 시뮬레이션 설계를 기록한다. 최초 런타임 검증은 부분 통과다.

[FIFO·RR 측정 및 판정 기준](fixed-cost.md) · [EVD-041 비용·스택 관측](evidence/evd-041.md)

[호스트 위상별 수용 분석](fs-m3-admission.md) · POL-004 #94의 CHG-045 / IMP-043. 조건부 호스트 산술과 CLI는 독립 재현을 통과했다. 런타임 추적 및 실측 예산 근거는 미확보다. [EVD-043](evidence/evd-043.md)

CHG-046 · [일반 Rust 작업의 두 가상 창 배치와 복구 기준](pol-006-rust-partition-plan.md) · [#99](https://github.com/Romanof123/JNOX/issues/99)

CHG-047 · [일반 Rust 서버 작업과 수용 기준](pol-007-rust-server-plan.md#korean) · [#101](https://github.com/Romanof123/JNOX/issues/101)

POL-004 / IMP-042. v3는 부분 통과다. 결정론적 정확 경계 두 건이 적격이었다. 기본 구성 23개 중 최초 14개, 콜드 재시도 한 건 포함 15개가 적격이었다. 기본 클록 일반 8개와 별도 기본 클록 인계 진단 4개는 미충족이었다. [EVD-042](evidence/evd-042.md)

CHG-049 / `IMP-050`. 최초 콜드 행렬에서는 진단 모드 96건 모두 초기화되지 않았다. 입력 수정 후 시작 검사 96건이 모두 통과했으며 수정 구성의 운영 적격 결과는 0/128이었다. [EVD-050](evidence/evd-050.md)

CHG-053 · [Ordinary control service and first dispatch / 일반 제어 서비스와 최초 디스패치](control-service-plan.md)

CHG-054 · REQ-002/005/023 · DES-003/012 · VER-002/005/023 · [일반 Rust 예약 작업·수용·실행 시간 검증](pol-008-rust-reservations.md) · [#123](https://github.com/Romanof123/JNOX/issues/123)

POL-006 / IMP-044. v2의 새 정상 콜드 실행은 P 10/10, Q 13/13 경계에서 1 ms를 충족했다. v1은 P 9/10, Q 11/12 경계가 초과했고 Q가 준비된 B 구간을 건너뛰었다. 가상 타이머 통지를 지연한 네 콜드 실행은 주입 경계에서 원래 1 ms 기준에 실패하고 지정된 창으로 복구했다. 그 밖의 지연 초과는 없었다. 실제 작업 마스킹은 미검증이다. [EVD-044 · 통지 지연](evidence/evd-044-notice.md) [EVD-044 v2](evidence/evd-044-v2.md)

POL-006 / CHG-050 / IMP-047. 일반 이미지와 진단 이미지는 각각 한 번의 콜드 부팅에서 IRQ 6회 후 PASS에 도달했다. 의도적 음성 대조군은 첫 A 마커에서 조건 99, 실제값 0, 기대값 1을 기록했다. 기존 PART FAIL의 원인은 미확정이다. [EVD-047](evidence/evd-047.md)

CHG-057 · [일반 Rust 복권·스트라이드 작업 기준](ordinary-proportional-plan.md) · [#54](https://github.com/Romanof123/JNOX/issues/54)

CHG-067 · [일반 Rust 뮤텍스 작업의 배치·소유권 기준](ordinary-mutex-plan.md) · [#49](https://github.com/Romanof123/JNOX/issues/49)

[POL-013 Pi 4 두 코어 보드 검증 계획](boards/pi4-smp.md)

[POL-013 Pi 4 작업 이동과 고정 대조](boards/pi4-smp-migration.md)
