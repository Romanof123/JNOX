# Corrected Pi 4 collector host verification / 수정된 Pi 4 수집기 호스트 검증

**PASS / 통과.** Tested detached commit a7f1a2487593722cd930a472f4eb0a98ea941906, tree 78768a5f476c7a80a57cb12182afe194992894c8. The original failing archive remains separately sealed, SHA-256 1bba1c3dbbca1b9f6545b9fbb9b26120f66992dd504453d868e45b6afc696759.

Windows Python ran 17 native and legacy serial tests. Four authenticated EVD-129 model UART files (FIFO and RR, plain and traced) passed through real file capture and again through the ordinary serial CLI with an injected 31-byte adapter. All eight captured streams matched their corresponding originals. The file sessions identify file_replay; the port sessions carry a simulated board revision and operator-declared source. Session records bind the pinned package manifest, both kernel artifacts, ordered app hashes and firmware. FIFO reported zero rotations, RR 16, dropped zero, exits 12 and 77. No physical board was available. These are host regression and model evidence.

For each of two RR serial attempts, the third read raised a controlled exception after delivery of 62 bytes. The on-disk result matched the raw prefix SHA-256 and two-read count; identity matched the run and session. OSError recorded read_error, read operation, type and message, exit 2. KeyboardInterrupt recorded interrupted, read operation, type and message, exit 130. Exactly one adapter opened per attempt and no second-run directory was created. The driver, console logs, raw bytes and collector JSON accompany this report.

네이티브·기존 시리얼 테스트 17개가 통과했다. EVD-129 모델의 FIFO·RR 일반 및 추적 UART 원본 네 개는 실제 파일 입력과 31바이트 단위의 모의 포트 입력에서 모두 일치했다. 세션 기록에서 패키지·커널 ELF·부트 이미지·순서가 고정된 앱 해시와 펌웨어를 확인했다. FIFO 교대는 0회, RR은 16회였고 누락은 0건, 종료값은 12와 77이었다. 실물 보드는 사용하지 않았다.

62바이트 뒤의 연결 끊김·사용자 중단 모두 디스크 결과에 부분 바이트 해시·읽기 횟수·오류 사유·실행 식별자가 보존됐다. 종료 코드는 각각 2와 130이었다. 각 시도에서 어댑터는 한 번만 열렸으며 두 번째 실행 디렉터리는 생성되지 않았다. 1차 실패 자료는 별도로 보존했다.

[Raw host captures and replay results](pi4-native-serial-corrected.zip) (SHA-256 `56c8c5f6ce99722efd34a162529ed30fa80c31842d03d733a9abccaab4c9f469`).
