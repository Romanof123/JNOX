# MG4 diagnostic positions

Use native Linux and the same pinned stock QEMU as the scored freezes. Create both scored freeze directories before freezing the diagnostic cohort. Inputs are the short clustered and stream global releases in that order. The helper copies each input into a separate new cohort directory. Record the printed cohort digest before D1.

```sh
python3 tools/pi4_mg4_terminal.py freeze NEW_COHORT SHORT_CLUSTERED SHORT_FREEZE_SHA STREAM_GLOBAL STREAM_FREEZE_SHA
python3 tools/pi4_mg4_terminal.py run NEW_COHORT COHORT_SHA D1
python3 tools/pi4_mg4_terminal.py run NEW_COHORT COHORT_SHA D2
```

Each position consumes its directory once. Keep every original and receipt even on a nonzero exit. Start D2 after D1 produces its receipt, including a partial receipt. Reopening D1 and its frozen inputs is part of the D2 command. The session records the quiet window, terminal text, hashes and exit status. The stopped CPU state, journal, stacks and fixed terminal object are diagnostic evidence. Read both phases and corroboration limits from the receipt. A captured object alone cannot establish an IRQ or ERET outcome.
