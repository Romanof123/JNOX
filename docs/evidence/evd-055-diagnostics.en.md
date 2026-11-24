# First ordinary proportional diagnostic captures

CHG-057 · POL-010 · EVD-055

One cold QEMU virt Cortex-A72 guest ran each predeclared configuration from 4a3eccd83b49a7b74d25f9e409c8c1dbe09781f2. Both used the default 62.5 MHz counter, recorded PPI 30 and stopped at the first grant. Rust 1.95.0 built the AArch64 ELFs on Windows following the EVD-055 capture setup. QEMU and the raw recorder ran in native Linux temporary directories. Each guest emitted a complete terminal record. The external log contains two IRQs and two ERET destinations per guest, covering the idle and client dispatches.

| Image | Owner | Start / compare / stop | Whole units + remainder (counts) | Worker progress |
|---|---|---|---|---:|
| lottery-miss | B | 721262 / 1971262 / 1978007 | 2 × 625000 + 6745 | 5347472 |
| stride-delay | A | 2580553 / 7580553 / 7591590 | 2 × 2500000 + 11037 | 24611280 |

Lottery seed 1 produced raw word 10451216379200822465 and ticket 2 in B’s interval [1,3). The compiled diagnostic requested two ticks. The original one-tick predicate recorded the expected rejection at slot 0, source line 427, actual 2 and expected 1. Stride admitted only weight-one A. Its pass rose from zero to 1,441,440 for the two observed units. Source line 425 requires at least three whole units. Footer 425/4/0/0/1 reports the Boolean result 0 against expected 1. The three-unit charge remains unobserved. Both inactive clients retained their prior progress, and independent replay found no UART syntax errors. The three earlier regular workloads keep their original results. Physical service and the historical four-IRQ failure remain open.

[ZIP](evd-055-diagnostics.zip) · [JSON](evd-055-diagnostics.json) · [EVD-055](evd-055.en.md)
