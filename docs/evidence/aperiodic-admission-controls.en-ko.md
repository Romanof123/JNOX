# Independent verification of #203 / #203 독립 검증

Head e7e202da4038be86de82375c8880c0d0e9a8b885. Prospective contract fae0e5ad46d8cf94e3fdfdd4d93177fb98dca987, docs/pol-007-rust-server-plan.md. Implementation in kernel/build.rs, kernel/src/aperiodic_admission.rs, kernel/src/ordinary_server.rs. Trace REQ-002/005/023 → DES-003/012 → VER-002/005/023. [PR #203](https://github.com/Romanof123/JNOX/pull/203) · [primary #101](https://github.com/Romanof123/JNOX/issues/101) · [parent #21](https://github.com/Romanof123/JNOX/issues/21).

Windows MSVC and Ubuntu x86_64 Rust 1.95.0 each passed 102 independent host parser vectors. Each platform completed three expected successful and five expected rejected Cargo release builds targeting aarch64-unknown-none, one job at a time. The synthetic pinned report exercises byte identity and arithmetic only. No qualified externally reviewed bounds were available.

On the single Cortex-A72 virt/GICv2/TCG 128 MiB guest, a-ps admission without inputs printed DENIED 1 after READY and emitted zero workload events. Default a-ps started normal workload events. Both initial QEMU invocations failed before boot due to missing efi-virtio.rom. Disabling the unused NIC allowed cold boots. The first effective default boot timed out mid-UART at five seconds, a subsequent 18-second stdout capture also truncated. A further cold boot of the same ELF with UART on ext4 completed AP END 0. Each launch outcome is retained. The complete verdict applies only to that final run. These results support the scheduled controls. Real admission and board/product WCET or ASIL remain unverified.

Windows와 Ubuntu에서 실제 Rust 판정기 102건씩 통과했다. 각 환경의 실제 Cargo 빌드 8건 중 정상 3건과 의도적 거부 5건이 예상과 일치했다. 게스트 a-ps는 보고서 부재 시 READY 직후 사유 1로 거부했고 작업 이벤트는 0건이었다. 최초 QEMU 호출은 ROM 부재로 부팅 전에 실패했다. NIC를 끈 최초 기본 모드 부팅에서는 실제 작업 이벤트가 나왔고 5초 제한으로 출력이 끊겼다. 18초 재실행도 출력 중단, 같은 ELF의 ext4 UART 재실행은 AP END 0까지 기록했다. 합성 입력은 계산 및 해시 검사에 한정한다. 실보드, 제품 WCET·ASIL, 실제 승인 상한과 전체 앱별 적시 완료는 미검증이다.

verify.py contains independent vectors and invocation details. Parser/build JSON records every expected and actual result. The first-boot and repeat logs retain partial or failed attempts. capture.py records the final UART invocation. Raw report bytes and both guest ELFs are included.

[Independent controls and original UARTs / 독립 시험과 최초 UART 원자료](aperiodic-admission-controls.zip) · SHA-256 f0195b37b226c1fc3dffec9941ab21e5f5ed70782b4651b1b166ac1bd31dc105.
