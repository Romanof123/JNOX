# First CHG-073 RR handoff captures

On source `702380d7797648cff740cc538bd301c442873e85`, both ordinary Raspberry Pi 4 model boots completed the RR transcript. Both engineering boots returned **2**. The HIGH interruption, return to A and advancing arithmetic were observed. Later timer rotation returned B to A in generation 5. B subsequently resumed in generation 6. The original loop-only criterion and the new same-generation handoff criterion both remained unobserved.

| Frozen position | Result | Positive mailbox payload | A interval, ticks | B interval, ticks |
|---|---|---:|---:|---:|
| O1 ordinary | Complete, 124 UART bytes | — | — | — |
| E1 engineering | RC2 | 1,820,706 | 649 | 23,211 |
| E2 engineering | RC2 | 1,871,585 | 642 | 20,488 |
| O2 ordinary | Complete, 124 UART bytes | — | — | — |

A released each payload within generation 5. B's single acquire returned that exact payload in each boot. Its surrounding observations spanned generations 4–6, including A's intervening dispatch, so the records cannot attribute the acquire to one dispatch. Independent exception ledgers matched all seven generation entries in each engineering capture. The linked STLR at `0x82208` stores the original operand. The LDAR at `0x82178` supplies both the branch and the captured value. Sampled SP distances below stack top were 0 bytes for A and 192 for B. High-water usage and physical-board timing remain unknown.

Compared with integrated v1 engineering, linked text grew by 452 bytes, read-only data by one byte and BSS by 176 bytes for two owner cells and two commit words. Data and reserved stack sizes matched. The ordinary raw image matched the integrated reference (`69db6275a2f6b2dbab0893e92495ce4d6bd3a7480027b1b16ddd5acf9c64beee`). Six malformed host copies scored 1. Replaying the original v1 FIFO/RR produced 0/0 and the integrated v1 FIFO/RR produced 0/2. All four new boots completed within the capture window and reached quiet. No guest was retried. Before O1, packaging found the source export lacked boards/pi4/config.txt. The archive retains the three original staging files. Adding the board files from the same commit allowed packaging to proceed.

[Capture archive](pi4-rr-handoff-first.zip) · [Manifest and results](pi4-rr-handoff-first.json) · [Contract](../boards/pi4-rr-handoff.en.md) · [한국어](pi4-rr-handoff-first.md). Extract the ZIP into an empty directory and run `python restore.py` there to reconstruct and digest-check every file.
