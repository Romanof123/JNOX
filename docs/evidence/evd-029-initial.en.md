# EVD-029 · CBS wakeup first invocation

[한국어](evd-029-initial.md) · [Acceptance contract](../m2-cbs-wake.en.md) · [#65](https://github.com/Romanof123/JNOX/issues/65)

The observer recorded **BLOCKED_BEFORE_BOOT**. The invocation could not find `/mnt/d/codex/target-cbs-wake`. It produced zero guest boots and created no capture directory. The verifier made no retry in this attempt. No UART, journal, exception trace or EDF replay exists for this invocation, so it provides no scheduling or timing verdict.

ELF SHA-256 `DFBB1D9E04C7219BC1C47FD96AF0F2465E1D924A5C64F5B03AC45757D4731706`. [Frozen source](https://github.com/Romanof123/JNOX/commit/93c64c853c3d1bf3f6286da59d83d7dc8394f7b9) · [Original verdict and wrapper](evd-029-initial.json)
