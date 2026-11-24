# CHG-071 · 첫 예약 단계 사전 검사

[CHG-071](../reservation-stage-attribution-plan.md) · [#153](https://github.com/Romanof123/JNOX/pull/153) · [Parent](https://github.com/Romanof123/JNOX/tree/ff9bf821d280fead823b3edddd0a20ff21c9f6f0) · [Candidate](https://github.com/Romanof123/JNOX/tree/2b210259679ae9271852f221ba7ccbeae86c9088) · [JSON](reservation-stage-first.json) · [ZIP](reservation-stage-first.zip)

정렬 깨움 기준 이미지가 필수 로드 이미지 동일성 검사를 통과하지 못했다. 예정된 게스트 12건 중 실행은 0건이다.

별도로 빌드한 부모 ELF는 과거 크기 최적화 정렬 깨움 ELF와 일치했다. 후보와 부모의 23,864바이트 .text는 동일했다. 두 PT_LOAD의 플래그, 주소, 크기 및 정렬도 각각 일치했다. 실행 가능 세그먼트 24,858바이트 중 .rodata 994바이트에 포함된 Rust 소스 위치 행 번호 10개에서 각각 1바이트씩 달랐다. JSON에는 주소와 리틀엔디언 값의 변경 전후를 기록했다. 쓰기 가능 세그먼트의 초기화 바이트와 영 채움 범위는 일치했다.

첫 검사는 W/B에서 중단됐다. CBS 포화 기준 및 E(EOI 직후)·F(두 번째 프레임 검증 직후) 이미지는 시험하지 않았다. 세 실행 묶음, 게스트 시간, 작업자 선택, 실물 측정 및 동적 비용 귀속 결과는 없다. 두 상세 빌드 로그, ELF, 추출 섹션, 역어셈블리, 심볼 맵 및 원본 비교 스크립트를 보존했다. 전체 소스 tar 해시는 SHA256SUMS에 있고, 원본 소스는 고정 커밋에서 확인할 수 있다. 공개 부분집합은 INCLUDED.sha256으로 확인한다.

```sh
unzip reservation-stage-first.zip -d first-stage
(cd first-stage && sha256sum -c INCLUDED.sha256)
```
