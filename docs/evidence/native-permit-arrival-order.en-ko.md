# #401 appendix · Live seq3 arrival at expiry / 활성 순번 3의 만료와 게시

**The same-tick gate remains open.** The six instruction-clock kernels built from `aa8e7e104033a372eed0675f3b76f7fb9af2a38e` retain the earlier proof for two independent services on one vCPU. Each boot shows the open-gate expiry CHECK for generation 1, sequence 3, with a prior output rise and a subsequent fall. The three calendars produced both strict event orders. In the two `arrival-same` boots the FIFO CHECK was at least 395 ticks after publication and the RR CHECK at least 419 ticks after publication.

The original authenticated object-code checker identifies the release store inside PID 0's SVC 34, the ordered counter reads immediately around its call and the protected PID 1 SVC 60 CHECK operand. The lower-A64 SVC vector at `0x40200c00` branches to `0x4020124c` where IRQs are masked. The successful second service always executes 16 contiguous SIMD pair stores at `0x402012c0…0x402012fc` before calling the dispatcher. The entire entry sequence before the call and the complete ordinary return sequence through ERET are byte-identical to the earlier kernels. The checker also confirms the call itself. The FIFO/RR images have different linked service addresses.

| Policy | Publish pre-read | Tail release store | Publish post-read | PID 1 CHECK operand |
|---|---:|---:|---:|---:|
| FIFO | `0x40204ab0` | `0x402069fc` | `0x40204ad8` | `0x40205f9c` |
| RR | `0x40204ab4` | `0x40206a00` | `0x40204adc` | `0x40205fa0` |

On this stock single-thread QEMU 6.2 fixed shift-0 clock those 16 guest instructions account for at least 16 virtual ns, or one 62.5 MHz counter tick. The A64 counter-read instruction ends its translation block. If publication runs first, its post-read precedes the controller's entry sequence. If CHECK runs first, its operand precedes the publisher's entry sequence and pre-read. Both arguments bracket the tail store using actual reads. This lower bound applies only to the six verified instruction-clock launches. The default-clock observations retain their empirical status.

| Boot | Seq4 publication bracket minus D | Actual CHECK minus D | Proven order and minimum gap in ticks |
|---|---:|---:|---|
| 007 FIFO before | [−61,684, −61,551] | +134 | Publication first, 61,685 |
| 008 FIFO same | [+392, +524] | +919 | Publication first, 395 |
| 009 FIFO after | [+62,781, +62,914] | +139 | CHECK first, 62,642 |
| 010 RR before | [−61,660, −61,528] | +141 | Publication first, 61,669 |
| 011 RR same | [+405, +537] | +956 | Publication first, 419 |
| 012 RR after | [+62,788, +62,920] | +146 | CHECK first, 62,642 |

D is the accepted seq3 wire issue plus 1,250,000 ticks. The protected CHECK operand comes from the matching expiry and decision records, with site 1, prior/new fault 0→1 and gate open, followed by PID 1/SVC 60/command 1/status 0. Event 11 identifies the successful next-sequence publication on handle 16. Each safety journal reported zero loss. Exact expiry at D with a valid frame already queued remains a distinct functional test.

**한국어.** 올바른 초기 상태를 갖춘 #401의 명령 수 기반 커널 여섯 개에서 원래의 명령어 근거를 다시 확인했다. 같은 CPU에서 두 번째 시스템 호출은 카운터 판독 전에 반드시 16개의 SIMD 쌍 저장 명령을 실행한다. 이 설정에서 최소 16 가상 ns, 즉 한 틱이다. 게시가 먼저이면 게시 후 판독과 CHECK 사이에, CHECK가 먼저이면 CHECK와 게시 전 판독 사이에 그 저장 명령들이 있다. 동일 틱 충돌은 열린 상태이며 기본 시계에는 이 하한을 적용하지 않는다.

여섯 캡처 모두 출력 상승 후 게이트가 열린 상태에서 순번 3 만료를 판정했고 출력이 하강했다. `arrival-same`의 실제 CHECK는 FIFO에서 게시 상한보다 395틱, RR에서 419틱 뒤였다. `arrival-after`에서는 두 정책 모두 실제 CHECK가 게시 하한보다 62,642틱 앞섰다. D 전에 정상 다음 프레임이 이미 대기할 때 정확히 D에서 만료를 판정하는 기능 시험은 별도로 남는다.

## Replay / 재검증

Use the authenticated original order packet and the restored #401 campaign root containing `runs/`. The appendix invokes the original checker unchanged. It also verifies the SHA-pinned #401 sessions, ELF, launch, UART and exact code ranges. All six kernel digests are in `arrival-witnesses.json`. The FIFO/RR `arrival-same` kernel SHA-256 values are `95194ae0add6331f70b41afd5f604545fd74f2da9e1721dc2a3a2b2470583eb1` and `1c5eef52798bdab2f67ba84390d2d1d97274e31514431b3e669f4335ee2bb21b`. Linux, Python 3.10+ and GNU AArch64 objdump suffice.

```sh
sha256sum -c SHA256SUMS
python3 -B check_arrivals.py "$ORDER_PACKET" "$ARRIVAL_ROOT" > replay.json
cmp arrival-witnesses.json replay.json
```

[Machine-code appendix and replay](native-permit-arrival-order.tar.xz) (SHA-256 `ab47a571a44f5d608046ed4ca39061bc8a83aeab814de42684ab4a7f8a46269b`). Use the [order proof](native-permit-collision-order.tar.xz) and the [confirmed enabled-state arrival campaign](native-permit-arrival-first.tar.xz) as inputs.
