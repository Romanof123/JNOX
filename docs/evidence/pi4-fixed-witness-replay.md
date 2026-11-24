# 수정된 Pi 4 증빙 디코더 · 호스트 재생

[JSON](pi4-fixed-witness-replay.json) · [ZIP](pi4-fixed-witness-replay.zip) · [#146](https://github.com/Romanof123/JNOX/pull/146) · [CHG-068](https://github.com/Romanof123/JNOX/commit/90980e3693a09f4be1efdd8440f1620264e1ee49) · [Decoder](https://github.com/Romanof123/JNOX/blob/60ed53d5c14cc33899f57b9285e585edc00daa72/tools/pi4_fixed_witness.py) · [CHG-063](pi4-fixed-witness-first.md) · [Inputs](pi4-fixed-witness-first.zip)

보존된 FIFO와 RR UART를 원래 디코더는 모두 코드 1로 거절했고, 수정 디코더는 같은 바이트에서 모두 코드 0을 반환했다. 새 게스트 부팅은 없었다.

FIFO는 선택된 EL1 복귀 10회, IRQ 2회, SVC 7회였다. RR은 EL1 복귀 7회, IRQ 3회, SVC 3회였다. 두 기록 모두 별도 EL2 부트 복귀가 한 번 있었다. 독립 분석에서 모든 선택 PC와 진입 ELR을 외부 예외 기록과 순서대로 대조했다. FIFO에서는 세대 4에서 HIGH가 실행된 뒤 5에서 A가 복원됐고 RR에서는 2 이후 3에서 복원됐다. 종료 시 유휴 ELR은 각각 0x82d70과 0x82d2c로 종료 행의 주소와 일치했다. 수정 도구는 선택 프레임·ELR·SP에 9–11열을 사용한다.

정책별 독립 제어 6건 중 프레임 주소를 복귀 대상으로 사용, 다른 코드 주소, 재개 주소 변경, 잘림, 알 수 없는 태그의 다섯 손상 사례는 코드 1이었다. 재개 후 진행 표본을 제거한 유효 입력은 미관측을 뜻하는 코드 2였다. 각 정책의 추가 왕복 양성 대조는 코드 0으로, 전체 제어는 14건이다. 아카이브에는 최초 호스트 설정 오류, 잘못된 프로필 호출, 실제 인수·종료값·출력·변조 바이트 및 독립 보고서가 있다. 초기 네이티브 환경에는 관찰 모듈, 이후에는 지원 nm가 없었다. 최종 재생에는 WSL Linux의 GNU AArch64 nm를 사용했다.

모델 예외 기록은 실제 복귀 PC를, 작업 표본은 양의 진행량을 뒷받침한다. 실물 보드 관측, 명령어 은퇴 측정 및 시간 상한은 미검증이다. 새 디렉터리에 두 ZIP을 두고 Python 3과 GNU aarch64-linux-gnu-nm으로 재생한다. 원본 재생 결과는 별도 디렉터리에 보존한다.

```sh
unzip pi4-fixed-witness-first.zip -d inputs
python3 inputs/restore.py
mv inputs/restored pi4-witness-independent
unzip pi4-fixed-witness-replay.zip -d archived-replay
(cd archived-replay && sha256sum -c SHA256SUMS)
cp -R archived-replay repeat
(cd repeat && python3 verify.py)
```
