# EVD-055 · 일반 비례 배분 통합 검증

통합 소스의 기본 클록 QEMU 첫 부팅 3건이 모두 완료됐다. 최초 EVD-055 집단과 별도로 집계한다.

[JSON](evd-055-integration.json) · [ZIP](evd-055-integration.zip) · [#136](https://github.com/Romanof123/JNOX/pull/136) · [Source](https://github.com/Romanof123/JNOX/tree/589e927bdc2e21ed68de1315d030d6231d18bacb)

| 사례 | A/B/C 실행권 | A/B/C 정수 서비스 틱 | IRQ/SVC/ERET |
|---|---|---|---|
| lottery1 | 10/8/14 | 10/8/14 | 33/0/33 |
| lottery2 | 5/11/16 | 5/11/16 | 33/0/33 |
| stride | 6/6/4 | 6/12/8 | 11/6/17 |

62.5 MHz 카운터에서 추첨 틱은 625,000카운트, 스트라이드 틱은 2,500,000카운트다. 독립 분석기는 시드별 추첨을 재현하고 유휴 복귀를 포함한 외부 예외 순서와 복귀 PC를 각 ELF와 대조했다. 스트라이드에서 처음 8회 서비스는 4/8/0틱이었다. C가 기존 작업자의 최저 패스 값으로 합류한 뒤 8회는 2/4/8틱이었다. 정수 서비스·진행량·프레임 지문은 게스트 기록이며 외부 추적은 예외와 복귀 주소를 확인한다.

전체 ELF 해시는 일반 이미지 3개와 RM 표식·기준 이미지 2개 모두 달라졌다. 다섯 이미지의 PT_LOAD와 .text는 각각 참조와 일치했다. RM 두 건은 빌드 비교만 수행했다. 제어 시나리오·보드·앱·프로필·벤치 혼용 오류 5종은 Cargo와 독립 선택자 양쪽에서 거절됐다. 네이티브 스케줄링 시험 130개를 통과했다. 첫 난수 증빙 하네스는 feedback_policy 누락으로 컴파일에 실패했다. 수정본은 0을 거절하고 두 번째 단어 e220a8397b1dcdaf를 수락해 티켓 2를 B에 배정했다. 양쪽 소스와 컴파일 로그를 보존했다.

증거 감사 지적은 0건이었다. 최초 아카이브 파일 해시 34개를 확인하고 이전 부팅 3건을 모두 재생했다. 추첨 누락·스트라이드 지연 진단은 실행하지 않았다. #79의 4 IRQ 사건 원인과 실물·제품 타이밍은 미확정이다. 아래 명령으로 원본 바이트를 확인하고 Python 3에서 통합 부팅 3건을 재생할 수 있다.

```sh
unzip evd-055-integration.zip -d integrated
cd integrated
sha256sum -c FILES.sha256
for k in lottery1 lottery2 stride; do
  python3 scorer/score.py score "$k" "guests/$k/uart" --elf "guests/$k/kernel.elf" --exceptions "guests/$k/exceptions" > "$k-replay.json"
done
```
