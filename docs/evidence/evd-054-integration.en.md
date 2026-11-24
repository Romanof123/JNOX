# EVD-054 · Integration gate

The integration gate repeated 32 frozen positions on the merged source. Zero of 20 operational positions qualified. Three A0 runs accepted seven permits each, but their first issue intervals were short. The original EVD-054 cohort retains its own denominator.

[JSON](evd-054-integration.json) · [ZIP](evd-054-integration.zip) · [#131](https://github.com/Romanof123/JNOX/pull/131) · [Source](https://github.com/Romanof123/JNOX/tree/2adccb563a525863e0f56c82703852082f5bc709)

The saved DAIF value in row 86 accompanies the guest pending-interrupt observation. Linked mask instructions and the external SVC/IRQ return sequence identify different parts of the path. Neither gives a bound for omitted mask tails or host intervention.


Commit 2adccb563a525863e0f56c82703852082f5bc709, WSL Ubuntu 22.04.3, QEMU 6.2.0, Rust 1.95.0. All 32 frozen positions reached terminal once. The original EVD-054 remains a separate 32-position cohort. The operational A1/A2 admission gate remains closed. No product timing or hardware qualification.

| Cohort | A0 | A1 | A2 |
|---|---|---|---|
| Operational default | 4 complete; 3 accepted seven without latch, 1 faulted before acceptance; all four short of the exact first issue gap; zero output edges | 8 refused | 4 refused |
| Operational icount | — | 2 refused | 2 refused |
| OFF default | — | 4 stopped | 4 stopped |
| OFF icount | — | 2 complete | 2 complete |

The sixteen positive phase images refused with mask 4095 before programming a compare or launching a worker. Seven default OFF runs latched a control service fault (103); RM 12-2 stopped for IRQ lateness (101). The first sender opportunity remained unattempted on all eight. All four deterministic OFF runs delivered seven distinct permits, with 625000-tick singleton first gaps, independent receiver acceptance, exact separate command at anchor +937500 ticks, four hypothetical eligibilities and zero modeled ON. For each boot seven SVC3 yields returned at the saved sender ELR via a subsequent IRQ; all 74 pre-ERET PCs matched actual return PCs.

The live host guard consumed QEMU PL061 events from reset. Before the first OFF boot, split-line, transient ON, masked-address aliases, direction/unrelated bits, malformed input and synthetic child-process stop controls passed. Each OFF boot recorded two zero writes and an initial zero on the QEMU output. The monitor sampled the append-only trace in its host loop, checked the remainder after termination, and sent SIGSTOP/SIGKILL on a detected alarm. No real alarm occurred. Host polling and buffering give no worst-case intervention bound. No guest-counter synchronization, physical pin reading, actual ON or fault-response interval was established.

| Position | Result | Latch / rejection | Issues / accepts | First issue gap | IRQ / SVC / ERET |
|---|---|---|---|---:|---|
| operational-rm-default-01-1 | observed-full-phase-horizon | — / — | 7 / 7 | 588087 | 26 / 33 / 60 |
| operational-rm-default-01-2 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-rm-default-10-1 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-rm-default-10-2 | observed-full-phase-horizon | [13, 263922, 3, 0] / — | 7 / 0 | 587136 | 26 / 33 / 60 |
| operational-rm-default-12-1 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-rm-default-12-2 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-rm-default-21-1 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-rm-default-21-2 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-dm-default-01-1 | observed-full-phase-horizon | — / — | 7 / 7 | 591691 | 26 / 33 / 60 |
| operational-dm-default-01-2 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-dm-default-10-1 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-dm-default-10-2 | observed-full-phase-horizon | — / — | 7 / 7 | 591544 | 26 / 33 / 60 |
| operational-dm-default-12-1 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-dm-default-12-2 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-dm-default-21-1 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-dm-default-21-2 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-rm-icount-12-1 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-rm-icount-12-2 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-dm-icount-12-1 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| operational-dm-icount-12-2 | refused-before-workers | — / — | 0 / 0 | — | 0 / 0 / 0 |
| characterization-rm-default-12-1 | observed-early-stop | [13, 293310, 3, 0] / [296343, 103, 0] | 0 / 0 | — | 1 / 1 / 2 |
| characterization-rm-default-12-2 | observed-early-stop | [13, 269339, 1, 0] / [272495, 101, 0] | 0 / 0 | — | 1 / 0 / 1 |
| characterization-rm-default-21-1 | observed-early-stop | [13, 293810, 3, 0] / [296892, 103, 0] | 0 / 0 | — | 1 / 1 / 2 |
| characterization-rm-default-21-2 | observed-early-stop | [13, 292350, 3, 0] / [295419, 103, 0] | 0 / 0 | — | 1 / 1 / 2 |
| characterization-dm-default-12-1 | observed-early-stop | [13, 285803, 3, 0] / [288805, 103, 0] | 0 / 0 | — | 1 / 1 / 2 |
| characterization-dm-default-12-2 | observed-early-stop | [13, 286509, 3, 0] / [291242, 103, 0] | 0 / 0 | — | 1 / 1 / 2 |
| characterization-dm-default-21-1 | observed-early-stop | [13, 291658, 3, 0] / [294711, 103, 0] | 0 / 0 | — | 1 / 1 / 2 |
| characterization-dm-default-21-2 | observed-early-stop | [13, 283012, 3, 0] / [286043, 103, 0] | 0 / 0 | — | 1 / 1 / 2 |
| characterization-rm-icount-12-1 | observed-full-phase-horizon | — / — | 7 / 7 | 625000 | 33 / 40 / 74 |
| characterization-rm-icount-12-2 | observed-full-phase-horizon | — / — | 7 / 7 | 625000 | 33 / 40 / 74 |
| characterization-dm-icount-12-1 | observed-full-phase-horizon | — / — | 7 / 7 | 625000 | 33 / 40 / 74 |
| characterization-dm-icount-12-2 | observed-full-phase-horizon | — / — | 7 / 7 | 625000 | 33 / 40 / 74 |

## Cross-profile and evidence results

Ten phase selector builds and all seventeen non-phase regression images compiled. Eight original invalid selectors and sixteen additional marker/cold conflicts failed compilation. All sixteen conflict logs contained the expected selector rejection. For both policies, A0, both operational phase images, both OFF images and the four #124 control arms differed from their archived loads only in decoded Rust source-location line fields. Addresses, flags, sizes, zero-fill and all other bytes matched. All nine #127/#112 partition images matched the earlier PT_LOAD records byte-for-byte. load-audit.json gives both digests and each decoded changed field.

All eight #124 live boots reached terminal. The two new marker journals decoded with exception corroboration. All four historical marker replays matched their original outputs, passed the revised checker and rejected injected duplicate debits. The healthy-only checker returned phase-envelope failures on RM baseline, marker and descriptor; DM baseline and marker failed actual delivery and DM descriptor failed rearm spacing. The healthy-only checker rejects cold row 48; the dedicated cold check verified both fixed no-probe boots. Separate mode-3 probes on RM and DM each executed one real probe, cleared pending and active bits and preserved the fixed epoch. These component results do not promote healthy operational status.

Both passive partition controls reached STOP within the check envelope. All four intentional notice-delay controls recovered after the prescribed late edge; independent checks found no unrelated late edge. The legacy bench and diagnostic passed; the intentional negative reported guard 99 and PART FAIL as expected. The scheduling evidence audit, nine partition accounting tests and Cargo partition selector regression passed.

The typed EVD-054 audit checked 32 ordered positions, aggregate field types, original source and byte-level outer ZIP, nested tar, both manifests and reference ELFs. Offline v1 replay regenerated the original results. All 19 v2 synthetic checks passed. On every archived position, v2 preserved outcome fields and matched the corrected IRQ intervals using row 2 absolute compare, row 3 requested index, and independent wake handling. V2 retains stronger per-interrupt provenance.

Run build.py, validate guard.py on Linux, then run.py; regression build, capture, scoring, typed audit and offline replay scripts are included. The first campaign remains the comparator. Early stops, cold checker exclusions and healthy timing failures remain recorded.

## Recover the original records

```sh
unzip evd-054-integration.zip
sha256sum -c archive.sha256
mkdir corpus
tar -xzf phase131-integrated.tar.gz -C corpus
(cd corpus && sha256sum -c SHA256SUMS)
```
