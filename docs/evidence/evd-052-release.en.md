# EVD-052 · Marker release image comparisons and six virtual boots

[EVD-052](evd-052.en.md) · [#124](https://github.com/Romanof123/JNOX/pull/124) · [3d314b0](https://github.com/Romanof123/JNOX/tree/3d314b0a188b1e93f1b2a6ad9ee35a47c30a7c9b) · [JSON](evd-052-release.json) · [ZIP](evd-052-release.zip)

All 13 built images matched their immediate ELF references exactly. Across 24 comparisons, 18 matched the entire ELF and loadable segments, and all 24 matched .text. Against #122, the six RM/DM baseline, descriptor and cold images each differed in 7–9 loaded bytes, all mapped to source-line metadata in control_runtime.rs. Four invalid selectors were rejected as expected: RM cold+descriptor, descriptor+marker, marker+cold and marker-loss.

Each of the six changed images ran once in a fresh QEMU process. All six reached terminal output. Operational positives were 0/6. Both RM and DM baselines failed the phase envelope, and both descriptor arms failed actual permit delivery. Both cold guests confirmed the fixed-epoch no-probe startup mode. The ordinary healthy scorer rejected diagnostic event 48 on line 4 of each cold UART. Those two rejections establish neither a diagnostic startup failure nor a separate measured operational timing violation.

The strict audit at ab14de0 returned HOLD (exit 1) for a dangling IMP-052 evidence path. After evidence integration, bf07551 and final source 3d314b0 each returned exit 0 with 18 policies and zero findings. All 147 file hashes in the original EVD-052 archive and the historical HOLD record digest matched. The four genuine marker journals in the earlier decoder study remain a separate cohort.
