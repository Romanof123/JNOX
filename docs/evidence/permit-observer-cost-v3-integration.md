# CHG-069 · 통합 호스트 원본 대조

[JSON](permit-observer-cost-v3-integration.json) · [ZIP](permit-observer-cost-v3-integration.zip) · [Source](https://github.com/Romanof123/JNOX/tree/e067c1a425cd4ec32760b70d53d01838aa3b165f)

독립 게이트는 16개 실행 위치별 원시 스트림 4개와 결과 1개, 총 80개를 네이티브 원본과 영구 사본에서 대조해 불일치 0건을 확인했다. 위치별 집계 영수증 16개를 검사하고 기록된 카운터 행으로 8쌍을 재계산했다. 송신 기회 112건은 모두 미시도였다. 이번 게이트의 새 게스트와 빌드는 각각 0건이다.

동결된 ELF 4개는 EVD-054 최초 아카이브와 일치했다. 이후 통합 EVD-054 이미지의 해시는 다르다. 연구·캡처 완료·병합 커밋에서 Python Git 블롭 6개가 일치했다. 실제 계측 파일의 줄 끝은 CRLF였다. LF 체크아웃만으로는 동결된 런타임 의존 파일 4개의 바이트 해시가 일치하지 않는다. 이 해시 재현에는 v3 아카이브의 계측 스크립트를 사용한다. JSON은 각 Git 블롭, LF 해시와 계측 CRLF 해시를 기록한다.

호스트 시험은 가드 6개, 종료·보존 7개, 증거 그래프 6개, 풋프린트 4개를 통과했고 그래프 지적은 0건이었다. 합성 자식 프로세스 6건으로 응답 또는 무응답 종료, 출력 전이, 예상 밖 읽기, 늦은 stderr와 조기 종료를 검사했다. 파일시스템 대조는 유효 집합을 보존하고 같은 크기 변조·추가 디렉터리·심볼릭 링크를 거절했다. 개인정보 대체 19건은 값을 공개하지 않고 원본과 대조했다. 이 비공개 대조를 반복하려면 네이티브 원본이 필요하다. 공개 호스트 제어는 Linux의 새 디렉터리에서 다음과 같이 재현한다.

```sh
unzip permit-observer-cost-v3-integration.zip -d gate
git clone https://github.com/Romanof123/JNOX.git gate/source
git -C gate/source checkout --detach e067c1a425cd4ec32760b70d53d01838aa3b165f
(cd gate && python3 -B controls.py)
```
