# CHG-071 · 예약 단계 두 번째 사전 검사

[CHG-071](../reservation-stage-attribution-plan.md) · [#153](https://github.com/Romanof123/JNOX/pull/153) · [JSON](reservation-stage-v2-first.json) · [ZIP](reservation-stage-v2-first.zip) · [Reference ZIP](evd-053-compiler.zip) · [v1](reservation-stage-first.md)

두 기준 로드 이미지가 일치했지만 E 이미지 둘 모두 링크된 명령 순서 조건에 실패했다. 예정된 게스트 12건의 실행 수는 0이다.

정렬 깨움(W)과 CBS 포화(S) 기준 이미지는 새 부모 빌드와 과거 참조 모두에서 PT_LOAD의 메모리 범위와 로드 바이트가 일치했다. 첫 세그먼트 크기는 각각 24,858·24,866바이트였다. 쓰기 가능 세그먼트는 모두 메모리 범위 134,704바이트에 초기화된 1,800바이트를 포함했다. 별도의 첫 사전 검사는 W 기준 이미지의 차이에서 중단됐다.

두 E 이미지 모두 최종 EOI DSB(0x402013e4) 바로 뒤에 CBNZ(0x402013e8)가 있었다. 분기 대상은 0x40201424였다. 분기하지 않을 때의 ISB(0x402013ec)와 카운터 읽기(0x402013f0)는 즉시 표본 기준을 충족하지 못했다. 두 F 이미지의 링크 목록에 ISB/MRS가 0x40201968/0x4020196c에 보인다. 이는 정적 배치 관측이며 실제 실행 여부는 확인되지 않았다. 호스트 재생도 수행하지 않았다. 모든 순번에서 UART, 작업자 결과, 동적 구간은 미관측이다.

같은 디렉터리에서 두 ZIP으로 매니페스트 51개 항목을 복원한다. 참조 ZIP은 공개된 EVD-053 컴파일러 아카이브와 동일하다. 두 소스 tar, ELF, 빌드 명령·로그, 심볼 맵, 전체 역어셈블리를 포함한 나머지 항목은 이 보충 자료에 무손실 저장했다.

```sh
unzip reservation-stage-v2-first.zip -d v2
python3 v2/restore.py evd-053-compiler.zip
(cd v2/restored && sha256sum -c SHA256SUMS)
```
