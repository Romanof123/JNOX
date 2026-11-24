# EVD-051 · 릴리스 이미지 및 추적 감사 비교

[EVD-051](evd-051.md) · [JSON](evd-051-release.json) · [ZIP](evd-051-release.zip) · [#122](https://github.com/Romanof123/JNOX/tree/caaaa3c3cf8bb22d9f592ffed9758492e6ff7aef) · [#124](https://github.com/Romanof123/JNOX/tree/bf07551c2cf999372e7437315f0358f2145031ee)

고정된 빌드 명령 13개에서 ELF 11개를 생성했다. RM·DM 각 기본·기술자·콜드 6개를 이전 통합 참조와 한 번씩 비교했고, 분할 P-short·Q-long과 기존 분할 3개는 각각 두 참조와 비교했다. 총 16/16에서 전체 ELF, 모든 PT_LOAD 배치·바이트, .text가 일치했다. RM의 cold+descriptor와 descriptor+marker 두 혼합 선택자는 예정한 오류로 종료 101을 반환했다.

#122의 caaaa3c와 #124 근거 커밋 bf07551에서 엄격 감사는 각각 정책 18개, 발견 0건, 종료 0이었다. #124의 이전 ab14de0에서 IMP-052 근거 경로 누락으로 기록한 HOLD(종료 1)는 보존한다. EVD-052 ZIP의 해시 대상 147개 항목과 원래 HOLD 기록 해시가 대조됐다.

이 릴리스 비교에서 게스트를 추가로 시작한 횟수는 0이다. 최초 독립 연구는 24회 중 22회가 첫 허가 전 억제됐고 두 기술자 DM 실행은 허가 7건 모두 수락한 뒤에도 OFF였다. 정상 시퀀스는 0/24였다. 이번 비교는 작업 기술자 비용 귀속이나 마스킹·보드 시간 상한을 새로 측정하지 않았다.
