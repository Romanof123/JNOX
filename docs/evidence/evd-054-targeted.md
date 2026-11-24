# EVD-054 · 위상·비례 배분 통합 대상 비교

[JSON](evd-054-targeted.json) · [ZIP](evd-054-targeted.zip) · [#131](https://github.com/Romanof123/JNOX/pull/131) · [#136](https://github.com/Romanof123/JNOX/pull/136) · [#142](https://github.com/Romanof123/JNOX/pull/142) · [Source](https://github.com/Romanof123/JNOX/tree/3b5a9f69034ecc2b7a7b093ddb786ee74f820be6)

허용된 릴리스 이미지 21개 모두 통합 참조와 로더 입력이 일치했고 전체 ELF 해시는 각각 달랐다. 음성 구성 44개는 모두 선택자에서 거절됐다. 총 65개 빌드에서 새 게스트 부팅은 0회였다.

위상 이미지 10개에는 이전 게스트 조건 32건이 대응한다. 제어 이미지 8개에는 기존 부팅 8건과 별도 모드 3 탐침 캡처 2건이 대응한다. 비례 배분 이미지 3개에는 lottery1·lottery2·stride의 실행권 32·32·16회 기록이 대응한다. 아카이브는 기존 실행 ID와 원자료 해시를 참조 이미지에 연결한다. 당시의 건강 조건 검사 실패와 승인 제한은 그대로다.

ELF 진입점, 세그먼트 플래그·주소·파일 크기·메모리 크기·정렬·초기화 바이트와 영 채움을 비교했다. 두 링크 인터페이스에서 벡터 기준, 복원 진입점, 실제 ERET 주소, 스택 경계, 16바이트 정렬, 작업당 16 KiB 스택과 832바이트 프레임 할당·복원을 대조했다. 엄격 그래프 감사 지적은 0건이고 그래프 시험 6개를 통과했다. 두 정책의 최초 및 통합 아카이브를 확인했다. 새로운 타이밍 측정 또는 운용 승인은 없다.

#142 호스트 도구 문서 연결을 위해 보관된 모든 .py/.sh 파일의 문자열을 별도로 확인했다. EVD-054 통합 내부 tar에는 run.py를 포함한 스크립트 14개가 있다. #136 최초·통합 ZIP에는 각각 3개·4개가 있고 각각 launch.py를 포함한다. 이 파일들과 EVD-054 최초 캡처 스크립트에서 qemu_boot와 fixed_runtime_observer 문자열은 발견되지 않았다. JSON에 경로·해시·검색 범위를 기록했다. 간접 의존성은 조사 범위 밖이다.

```sh
unzip evd-054-targeted.zip -d targeted
(cd targeted && sha256sum -c SHA256SUMS)
```
