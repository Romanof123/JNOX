# Pi 4 직렬 캡처 도구 · 호스트 검증

[JSON](pi4-fixed-serial-host.json) · [ZIP](pi4-physical-fixed-host-qualification.zip) · [#142](https://github.com/Romanof123/JNOX/pull/142) · [Source](https://github.com/Romanof123/JNOX/tree/ed611730a657b9cc7dda300626bcc8473faa52c9)

통합 호스트 수집기는 시간 조건 시험 566건, 관찰 API 검사 8건, 모의 포트 사례 4건과 기존 캡처 시험 7건을 통과했다. [EVD-014](evd-014.md) · [EVD-031](evd-031.md)

FIFO와 RR 재생에는 보존된 모델 UART 두 스트림과 별도 실행 ID를 사용했다. 원본 RR은 두 건 모두 rr-a-first였다. 별도로 구성한 쌍에서 rr-high-first도 확인했다. 명시적 및 기본 스케줄러 재생은 각각 동일한 과거 스트림을 두 번 사용했다. EVD-031 전체 아카이브와 내부 파일, EVD-014 원본 9건, 배포 ZIP 3개와 그 구성 파일을 SHA-256으로 대조했다. CLI 재생 전 원본 ELF와 kernel8.img도 확인했다. ELF나 패키지를 바꾸면 UART 일치 판정은 유지되지만 빌드 연결 차이가 기록됐다. ZIP에는 UART 원본과 패키지 해시가 있다. CLI 전체 재생에는 별도 원본 펌웨어 배포본과 ELF가 필요하다.

바이트 단위 수신과 모든 단일 분할점, 손상 프레임, 허용 RR 순서 둘과 금지 순서, 기한 경계와 무출력 대기, 수신 오프셋 및 읽기 실패를 검사했다. 관찰 API는 호스트에서 호출했다. 주입한 가짜 포트에서 두 번의 준비 절차, 조기 종료, 읽기 오류와 운영자 입력 종료를 확인했다. 중단된 세 사례에서는 최종 판정 없이 부분 식별 기록이 남았다. 이 기록의 물리 모드 표시는 CLI 선택값이다.

첫 하네스는 현지화된 하위 프로세스 stderr 처리에서, 다른 하네스는 해당 Python이 지원하지 않는 zip 키워드에서 중단됐다. 최초 오류와 재시도 기록을 모두 보존했다. 보드·EEPROM·카드·어댑터·입회자 식별값은 비어 있다. 실물 전원 재인가, USB 관찰, 실행 PC 증거, 최악 실행 시간은 측정하지 않았다.

```sh
sha256sum pi4-physical-fixed-host-qualification.zip
unzip pi4-physical-fixed-host-qualification.zip -d host-records
```
