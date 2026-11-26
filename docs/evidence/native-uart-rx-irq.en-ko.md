# PL011 receive interrupt results / PL011 수신 인터럽트 실측

The native Rust driver received serial input while the application slept on single-core AArch64 QEMU virt. The observer sent bytes to QEMU's PL011 chardev, read guest measurements through independent QMP framebuffer captures and checked boot identities and permission outcomes on UART. All three packages used the same native application ELF. [Design](../native-uart-rx-irq.en-ko.md) · [Build and run](../../examples/native-uart-rx-irq/README.md) · [Driver #414](https://github.com/Romanof123/JNOX/issues/414)

앱이 타이머로 잠든 사이 시리얼 데이터를 외부에서 넣어 측정했다. 관측 프로그램은 실제로 보낸 바이트와 QMP 화면에서 읽은 큐 내용, UART에 남은 종료 코드를 대조했다. IRQ·폴링·헤드리스 3개 이미지는 동일한 앱 ELF를 사용했다.

| Test / 시험 | Observed result / 관측 |
| --- | --- |
| 8 binary bytes | Read `00 fe 11 80 7f 55 0d a3` in order. Seven IRQ entries during CPU idle, zero dropped bytes or UART error words, 100 ms measured guest wait. / 8바이트 순서 일치, 유실·오류 0건. |
| A further 24 bytes | The first 16 bytes (`a`–`p`) survived and eight excess bytes were dropped. Cumulative valid bytes 32, IRQ entries 30, idle IRQ entries 30, error words zero. The second wait also measured 100 ms. / 앞의 16바이트를 보관하고 초과 8바이트를 기록했다. |
| IRQ owner and peer | Owner accepted the final quit byte. The peer saw permission denial. Exit codes 70 and 71. / 비소유자의 입력·통계 접근이 거부됐다. |
| Polled control and headless control | Default polled input delivered `p`; IRQ statistics were unavailable. Owner and peer exited 73 and 72. The headless app exited 38 for the unsupported interface. / 기본 폴링으로 실제 입력을 받았고 헤드리스는 정의된 미지원 결과를 반환했다. |

All three scheduler journals reported zero dropped records. The display boots emitted QEMU's optional VGA-ROM warning and returned usable ramfb captures. Error-word discard and the invalid-metric error are implemented; this experiment injected valid UART words. The measured waits demonstrate timer progress only. Driver replacement, physical IRQ latency, DMA and product safety qualification require separate evidence. / 실측 100 ms는 게스트 타이머가 진행했음을 보여준다. UART 하드웨어 오류 주입, 실보드 IRQ 지연, DMA 및 드라이버 복구는 별도 검증 범위다. [Next native recovery work #415](https://github.com/Romanof123/JNOX/issues/415)

## Reproduce / 재현

EVD-319 · CHG-319 · IMP-319 · REQ-015/016 · VER-015/016. Local build commit `a9e2528758d2a1d7e023d36b32485fa0d5514c6a` and [published source](https://github.com/Romanof123/JNOX/commit/29870ecaa9e14eb4158765b23ceab7e2f10e6069) share Git tree `7be3eb62bade146b86aa0da0f9a43e68d467cda8`. IRQ kernel SHA-256 `3412cb44a60b51582a2a436f515486db948f5f2e198c2fb536c2c3bc5d555791`. Download the [original packet](native-uart-rx-irq.tar.xz), verify SHA-256 `d1c14b943c048df293bc39f2b336424bd77bc3973dd4c6ebc25fd591046a3b79`, extract and run the commands from its root. / 압축 파일의 해시를 확인하고 원본 시험을 재판독한다.

```sh
sha256sum --quiet -c SHA256SUMS
python3 -B tools/verify_native_uart_rx_irq.py --replay capture
```
