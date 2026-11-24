# EVD-050 · Observer storage crossover

[EVD-050](evd-050.en.md) · [JSON / SHA-256 / archive](evd-050-storage.json) · [#118](https://github.com/Romanof123/JNOX/issues/118) · [#109](https://github.com/Romanof123/JNOX/issues/109)

Each of two workloads ran four cold boots per block across four blocks, for 32 boots. A wrote UART and exception logs to native WSL ext4. B moved only the exception log to DrvFs, C moved only UART, and D moved both. The 62.5 MHz guest counter measured handler entry. The IRQ0 limits were 0.5 ms for RM and 1 ms for A-PS.

| Stratum | Cell | First IRQ within limit | Healthy completion | Guest terminal |
|---|---|---:|---:|---:|
| RM | A | 4/4 | 0/4 | 4/4 |
| RM | B | 0/4 | 0/4 | 4/4 |
| RM | C | 4/4 | 0/4 | 3/4 |
| RM | D | 0/4 | 0/4 | 4/4 |
| PS | A | 4/4 | 4/4 | 4/4 |
| PS | B | 0/4 | 0/4 | 4/4 |
| PS | C | 4/4 | 0/4 | 0/4 |
| PS | D | 0/4 | 0/4 | 4/4 |

The table gives first-IRQ lateness in guest ticks. All 16 within-block comparisons at fixed UART route recorded greater lateness with DrvFs exception capture. UART contrasts at fixed exception route varied in sign.

|Stratum/block|A|B|C|D|C−A|D−B|B−A|D−C|D−B−C+A|
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
|RM 1|13787|147872|13340|136711|-447|-11161|134085|123371|-10714|
|RM 2|10074|129076|13157|115821|3083|-13255|119002|102664|-16338|
|RM 3|12550|114130|11747|115317|-803|1187|101580|103570|1990|
|RM 4|9924|113216|9637|105572|-287|-7644|103292|95935|-7357|
|PS 1|17074|128156|19692|146371|2618|18215|111082|126679|15597|
|PS 2|19510|125659|18207|140714|-1303|15055|106149|122507|16358|
|PS 3|23649|143309|16829|134298|-6820|-9011|119660|117469|-2191|
|PS 4|16517|140316|15080|136119|-1437|-4197|123799|121039|-2760|

RM accepted zero permits across 16 boots. A host ENODATA read aborted the observer in 1-RM-C. Post-exit recovery retained the raw trace, with terminal timing, process status and the next interlaunch gap unknown. All four A-PS C boots timed out during step output. Each recorded 17 IRQs and four submitted units, charges, server completions and periodic completions. Blocks 1–4 retained 201, 198, 166 and 196 step records. Return, full-step and terminal checks remained incomplete. The four A-PS A boots account for all healthy completions.

QEMU 6.2.0 used its default clock and file backends. Host polling targeted 10 ms and used a separate clock. RM adopted the common -nodefaults command and omitted the earlier -no-reboot option. The experiment cannot separate host activity, TCG and filesystem internals or establish physical board timing or repaired fw_cfg behavior.

Frozen plan and image sources · [#118](https://github.com/Romanof123/JNOX/blob/79ec3edc390bf40eca65935ec0baa2b2ed5467e1/docs/control-cold-plan.md) · [RM](https://github.com/Romanof123/JNOX/tree/e0f0ce623aa170a8daff600515b0c76e0b20405f) · [PS](https://github.com/Romanof123/JNOX/tree/86457068e7ba019d3deabb80f63f47d50436c4a6)
