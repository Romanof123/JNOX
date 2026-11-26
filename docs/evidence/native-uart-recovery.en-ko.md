# UART driver handoff and peer continuity / UART 드라이버 교체와 앱 연속성

One AArch64 QEMU virt FIFO image ran a primary driver, a continuing application, and a dormant driver with three private address spaces. The primary explicitly reported an error by exiting with status 86. The kernel masked UART/GIC input, revoked the primary and admitted the standby once. Initialization cleared stale input before the new owner received fresh bytes. A second reported failure left the interrupt disabled. On normal primary exit the same image dismissed the spare without executing it. [Design](../native-uart-recovery.en-ko.md) · [Run the example](../../examples/native-uart-recovery/README.md) · [#415](https://github.com/Romanof123/JNOX/issues/415)

주 드라이버가 오류 코드 86으로 종료하면 예비 드라이버가 권한을 한 번 승계했다. 두 드라이버 사이에서 일반 앱은 계속 실행됐고 종료 시까지 50회 진행했다. 정상 종료 시험에서는 예비 드라이버가 실행되지 않았다.

| Phase / 관측 단계 | Stronger failure run / 장치 FIFO까지 관측한 오류 종료 시험 |
| --- | --- |
| Primary | Four previous payload bytes remained in the software queue before a final handoff marker. The peer had recorded its first tick. |
| Revocation | QMP read PL011 IMSC = 0 and GICv2 INTID 33 disabled. After the host sent `d3`, UARTFR changed from `0x90` (RX empty) to `0xc0` (RX occupied). Both interrupt masks remained off. |
| Replacement | Standby initialization restored IMSC = `0x50` and the GIC enable bit. UARTFR returned to `0x90`. Software depth, accumulated traffic and error counters started at zero. The new owner returned exactly the freshly injected `e7 a5 df f4 6c 6b 8b 4a`. The peer advanced to tick 7 in the same address space. |
| Spare exhausted | A second exit 86 disabled both interrupt masks. The peer kept running to tick 50. UART exited slots 0/1/2 with status 86/71/86. |
| Normal control | Primary exited 0, peer reached tick 50 and exited 71. The kernel reported slot 2 dismissed. The interrupt was disabled. |

권한 회수 중 실제 UART에 `d3`를 넣어 하드웨어 수신 버퍼가 차고, 예비 드라이버 초기화 시 비는 상태를 외부에서 읽었다. 예비 드라이버는 새로 보낸 8바이트만 받았다. QMP로 세 프로세스의 페이지 테이블을 각각 따라가 독립된 메모리에서 진행 상태를 읽었다.

The initial host attempt stopped before guest launch because the package manifest lacked the three expected MMU roots. The packager now emits them from the linked kernel. The first corrected pair of boots verified software backlog and peer continuity. A second pair on the same kernel and application binaries added the hardware FIFO observation. The scheduler journal reported 201 drops in the detailed failure run and 161 in its normal control. UART, independent page-table reads, register samples and framebuffer records supplied the stated verdicts. General CPU exceptions, DMA reclamation and physical timing remain open under [#5](https://github.com/Romanof123/JNOX/issues/5). / 첫 호스트 검증은 매니페스트에 주소 공간 정보가 빠져 부팅 전에 중단됐다. 이를 고친 뒤 네 번 부팅했다. 일반 CPU 예외 격리, DMA 및 실물 안전 승인은 별도 범위다.

## Evidence / 증거

EVD-320 · CHG-320 · IMP-320 · REQ-015/016 · VER-015/016. Local `c97601453040cedb6ce25e8d13a7ab74ff70ee2e` and [published source](https://github.com/Romanof123/JNOX/commit/cfbfa710f2698824cc7d2781547cd86c4f624349) share tree `c06fb56eca78d59c6ffe0b6f34a3a2f91611cae0`. Kernel `42b04deab2c680fdab94e2ebd734b5f80ca9e86b826fc5e3f65bb34ea38326fe`. The PM retained the original boot archive and both observers locally, SHA-256 `f754e43870e7ff50011c1aeed0b3cedf9a6bf8c7aae49e1ff93753c3ad6f3dd2`. The example above reproduces the experiment from the published source. With a local copy of the archive, extract it and run these commands on Linux with Python 3 and AArch64 GNU binutils. / 원본 압축은 로컬에 보존했다. 링크된 예제와 코드를 사용하면 새로운 부팅 시험을 재현할 수 있다.

```sh
sha256sum --quiet -c SHA256SUMS
python3 -B tools/verify_earlier_recovery.py --replay control-observer/capture
python3 -B tools/verify_native_uart_recovery.py --replay capture
```
