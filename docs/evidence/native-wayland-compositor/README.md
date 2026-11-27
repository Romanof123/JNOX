# JNOX #446 resident Wayland capture

Source revision `35f531a0f2f088cbcf1baaa7150e18570641d0e8`. Two cold Cortex-A72 QEMU TCG runs exercise two native clients, the compositor and local socket service. Each run contains six external 320×240 PPM scanouts, six matching PNGs, seven stopped physical-page archives, UART and QMP events. The sequence is initial → A responds → B gains focus → B responds → B terminates → A responds again. The subsequent quit produces the terminal snapshot. The expected exits in PID order are 0, 71, 72, 0. Fixed UART event buffers reported 11,708 and 11,884 dropped entries.

The Linux reference client uses Ubuntu libwayland-client and libxkbcommon. The host harness externally located all 320 client pixels at (16,24) and (13,22) with the offset geometry. Both host logs, process maps, wire traces, reference executables, generated code, external mapped frames and final framebuffers are included. The guest uses its native clients. The guest evidence covers the observed protocol subset, shared page mappings, software scanout and input sequence on this virtual board. The transient guest keymap bytes, hardware graphics, compositor restart, concurrent headless workload timing and general malformed-peer handling remain unverified by these captures. The physical archives contain the pages selected by the recorder. Earlier builds' timeline and compiler failure are included under history.

## Replay on Linux

Use Python 3.10 or newer, GNU nm and sha256sum. Download the archive, its SHA256SUMS and extract.py into a fresh working directory. If issued as parts, concatenate them in filename order before checking the archive digest.

```sh
# For split distribution only
# cat jnox-446-35f531a.tar.xz.part* > jnox-446-35f531a.tar.xz
sha256sum -c SHA256SUMS
python3 -B extract.py jnox-446-35f531a.tar.xz unpacked
python3 -B unpacked/replay.py
```

The replay checks the capture inventory, runs the capture-time and reviewed observers, mutates two input witnesses and a scanout pixel in temporary copies, decodes all twelve PNGs and repeats the host pixel-origin search. Executables are stored as evidence. Public SHA-256 values allow comparison against a separately trusted release record.

## 한국어

위 커밋에서 만든 QEMU 게스트를 두 번 냉간 부팅해 클라이언트 A/B의 입력, 포커스 변경, B 종료 뒤 A의 반응과 최종 종료를 기록했다. 부팅별로 여섯 시점의 원본 PPM 화면과 동일 픽셀 PNG, 일곱 시점의 선택된 물리 페이지, UART 및 QMP 기록이 있다. UART 고정 버퍼의 누락 건수는 각각 11,708건과 11,884건이다.

리눅스 호스트에서는 배포판의 Wayland 및 XKB 라이브러리를 사용하는 참조 클라이언트로 두 가지 창 배치를 검사했다. 외부에서 읽은 프레임에서 320픽셀 전체의 시작 좌표는 (16,24)와 (13,22)였다. 게스트 실험에는 네이티브 클라이언트를 사용했다. 실물 장치, 게스트의 일시적인 키맵 원문, 컴포지터 재시작과 동시 백그라운드 작업 지연은 이 기록의 검증 범위 밖이다. 위 명령을 실행하면 원본 파일 해시, 두 버전 관측기의 결과, 손상 사본 거부, PNG 픽셀 및 호스트 프레임 위치를 다시 확인한다.
