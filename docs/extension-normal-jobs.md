# Ordinary bounded extension jobs (#221 / #21, POL-018)

Use the existing one-core nine-slot counter schedule, five VM fuel units, one-slot grants and one independent reservation every third slot. Link a finite Rust sensor producer, a consuming accumulator and an independent periodic checksum. A single IfReady program nominates the producer when its one-element mailbox is empty. Otherwise Yield lets the trusted fallback service the consumer. Each service records its result once per epoch. Replay the three mailbox handoffs and the three independent samples from the linked inputs. Compare fresh entry and subsequent saved dispatch PCs, timer budgets, fuel and reservation timing against the existing extension contract. Keep legacy image selection and capture layout. Verify normal UART on a plain run, then compare a fresh capture from the same ELF. These are prospective QEMU checks.

# 일반 확장 작업 계획

기존 단일 코어의 아홉 슬롯, 연료 다섯 단위, 한 슬롯 승인, 세 슬롯마다 독립 예약 한 번을 유지한다. 연결된 Rust 생산자가 빈 우편함에 센서 표본을 넣으면 같은 IfReady 프로그램이 다음 차례에 Yield를 실행하여 소비자가 누적값을 계산한다. 독립 예약 작업은 별도 입력의 검사합을 갱신한다. 슬롯별 결과와 세 번의 전달, 예약 시각, 실제 저장 PC에서의 재개, 연료와 승인량을 독립 재생으로 확인한다. 기존 이미지 선택과 캡처 형식을 유지하며 같은 ELF의 일반 실행과 캡처 실행을 비교할 예정이다.

## Maintainer reproduction / 재현 절차

From the repository root on Linux, use a new capture directory for each run. The checked-in replay expects the unstripped ELF and GNU AArch64 nm. The legacy image continues to use the extension-bench feature, extension observer and extension_capture.py.

```sh
(cd kernel && cargo build --release --locked --features extension-normal-jobs -j1)
elf=kernel/target/aarch64-unknown-none/release/jnox-kernel
python3 tools/qemu_boot.py --profile extension-normal --kernel "$elf"
run=$(mktemp -d)
export EXTENSION_CAPTURE_DIR="$run/capture"
cat > "$run/qemu" <<'SH'
#!/bin/sh
exec python3 -B tools/extension_normal_capture.py "$@"
SH
chmod +x "$run/qemu"
python3 tools/qemu_boot.py --profile extension-normal --kernel "$elf" --qemu "$run/qemu"
python3 tools/extension_normal_replay.py "$EXTENSION_CAPTURE_DIR" "$elf"
```

Both boots must match the same strict UART transcript. The captured boot additionally retains the kernel schedule journal and a separate worker-written result per epoch. With temperature inputs 18, 23, 21 the consumer must produce 18, 41, 62. The independently reserved voltage accumulator must produce 3300, 6596, 9900. At the last reservation neither delegate has remaining work. No extra delegated slot is released. The interrupt PC is sampled in the service loop after each finite Rust update. Later dispatches of that job must restore that sampled PC and frame. The capture does not count Rust instructions or measure physical hardware.

일반 실행과 캡처 실행 모두 같은 UART 기록과 일치해야 한다. 캡처는 온도 누적합, 독립 전압 누적합, 작업별 프레임과 재개 PC, 각 슬롯의 카운터 및 승인량을 비교한다. 마지막 예약 시 두 위임 작업은 모두 완료 상태다.

[First independent verification and original captures / 최초 독립 검증과 원본](evidence/ordinary-extension-jobs-first.en-ko.md).

2026-11-25 · Retrospective trace / 사후 추적 · [#221](https://github.com/Romanof123/JNOX/issues/221) · [#223](https://github.com/Romanof123/JNOX/pull/223) · [EVD-093 (EN)](evidence/evd-093.en.md) · [EVD-093 (한국어)](evidence/evd-093.md) · [#239](finite-scheduling-index.en-ko.md)
