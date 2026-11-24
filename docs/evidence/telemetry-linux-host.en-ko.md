# Linux telemetry reference / Linux 텔레메트리 기준 실행

All 17 published cases at `952518ecfe25c1d098021dc916d1174392cba77b` matched stdout, stderr and decimal status bytes. GCC 11.4.0 on Ubuntu 22.04.3 x86-64 (WSL2) compiled the original telemetry_monitor.c with `cc -std=c99 -Wall -Wextra -Wpedantic -Werror`. The independent oracle recomputed valid summaries from inputs and compared every stream. In split-token, 65535 occupies offsets 254–258 across the 256-byte read boundary. The 128 readings yielded min 1, max 65535 and sum 65662. A separate valid input with stdout redirected to /dev/full returned 7 and wrote exactly `output error\n` to stderr. The archive includes case inputs, argv, raw observations and the host executable. Extract on Linux for the case named nul, then run `python3 oracle.py`.

공개된 17개 입력 모두 표준 출력·표준 오류·종료 상태가 일치했다. 별도 검산기로 실제 입력에서 정상 사례의 요약을 다시 계산했다. 256바이트 경계를 가로지른 숫자 65535를 포함한 128개 표본의 합은 65662였다. 추가 /dev/full 시험은 종료 코드 7과 정확한 오류 메시지를 남겼다. Windows 및 WSL의 활성 경로에서 QNX qcc를 찾지 못했다. 이번 관측 범위는 Linux 호스트이며 QNX와 J-NOX에서의 실행은 미측정이다.

Source SHA-256 `c7e59f566052c8caa9f1d4d1b5560fd4f339d4ab20172e826073296c2f51ad28`. Linux ELF SHA-256 `0bce56a4deb9f0c95860256c79a31735ba6f6323805382fa1352c68b21727644`. ZIP SHA-256 `b4ed6312e7f9d3789ac0023d18f0c5feca4baebea2f8e6e95ab7a07b70fc47de`. [Observations and oracle / 원시 관측 및 검산기](telemetry-linux-host.zip)
