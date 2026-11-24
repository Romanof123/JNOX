# Pi 4 MG2 최초 여섯 번의 마이그레이션 실행

세 배치 방식 모두 최초 두 번의 부팅에서 M이 마이그레이션 IRQ 전에 종료됐다. MG2의 엄격한 루프 중단 판정은 **0/6**이다. 과거 MG1 기록은 partitioned 1만 통과해 **1/6**이었다.

CPU별 예외 로그에서 CPU0의 M 종료 SVC와 CPU1의 첫 P1 SVC를 확인했다. UART의 0x1d 분기는 M의 합계 2147516416을 검사한다. 마이그레이션 복귀 PC, X21, 네 활성화의 체크포인트 요약은 관측되지 않았다. 각 실행은 QEMU 11.1.3 raspi4b 모델을 사용했다.

| Placement | Positions | CPU0 M SVC ELR | CPU1 P1 SVC ELR |
|---|---|---|---|
| global | 1, 2 | 0x81f9c | 0x82098 |
| partitioned | 1, 2 | 0x81fa0 | 0x8209c |
| clustered | 1, 2 | 0x8204c | 0x82148 |

호스트에서 만든 정상 대조 기록은 통과했고, 개수·해시·인덱스·합계·리스·예약 필드를 변경하거나 요약을 제거한 일곱 변형은 모두 거부됐다. 압축 파일에 오프라인 재현 명령과 공개용 경로 정규화 전후의 해시를 첨부했다.

[MG2 contract](../boards/pi4-smp-migration.md) · [MG1](pi4-migration-first.md) · [JSON](pi4-migration-mg2-first.json) · [ZIP](pi4-migration-mg2-first.zip)

[Integrated image verification / 통합 이미지 검증](pi4-mg2-integrated.en-ko.md)
