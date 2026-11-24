# CHG-071 · First v3 reservation stage captures

[CHG-071](../reservation-stage-attribution-plan.md) · [#153](https://github.com/Romanof123/JNOX/pull/153) · [Source](https://github.com/Romanof123/JNOX/tree/df8fde7f5241a722724f06b073ea3fb53e701659) · [JSON](reservation-stage-v3-first.json) · [ZIP](reservation-stage-v3-first.zip) · [v1](reservation-stage-first.en.md) · [v2](reservation-stage-v2-first.en.md)

Aligned wake (W, plain) completed 3/6 first boots. Saturated CBS (S, traced) completed 2/6. No baseline boot reached its horizon. The opening guard remained 62,500 ticks (1 ms).

All twelve positions are retained, including four pre-selection stops and three late worker markers. W completions reached slot 34 with 18 grants; S completions reached slot 10 with eight. E and F split the observed interval at different points. In both repeats of each stratum the E suffix and F prefix were larger. None of the four split comparisons met the range and stage-reach criteria for transfer to B. These measurements therefore leave the baseline cost split unresolved.

Source df8fde7f5241a722724f06b073ea3fb53e701659. Both full B ELF hashes differ from the historical files. Both baseline selectors matched both historical and independently rebuilt aacdbebe958feca152b358ce8cf36033cd7d6749 references, all load bytes and segment headers. Both E ELFs store IAR at 0x402013d4. CBNZ selects the ordinary single DSB at 0x40201428 or the single sampled DSB/ISB/MRS at 0x402013e8/ec/f0, witnessed by ADR at f4. Sampled branch skips the alternate barrier. In each F ELF the frame check loop advances by eight to 16, failures reach the nonreturning call at 0x40201964. Successful second exit at 0x4020185c reaches ISB/MRS 0x40201968/6c and ADR at 70 before destination-bank access. Each B has 26 CNTPCT instructions, E/F have 27. Whole linked images, section maps, probe and source manifests are in the archive. Prior refusals are separate, with zero guest launches apiece.

QEMU virt A72, one CPU, TCG, 128 MiB, default clock. Each guest retained the full first 15 s timeout and terminal UART. W plain, S external IRQ/SVC/ERET trace. Frequency 62,500,000 Hz, original 62,500-tick opening bound. Code 2/5 is an opening-guard stop before selection; 2/2 is a late worker marker. Detailed actual stage receipts, operands, marker, grants, real returns, per-owner cumulative service and linked exception audit are in per-position audit.json. A dash denotes an unobserved endpoint.

| Position | Mask | terminal slot/owner/status/reason | guard operands | IRQ−epoch | reads | prefix | suffix | whole | marker−epoch | grants |
|---|---:|---|---|---:|---:|---:|---:|---:|---:|---:|
| 01-WB | 3 | 0/2/2/5 | 3287591 / 3210074 | 37327 | 10479 | — | — | 29711 | — | 0 |
| 02-WE | 15 | 0/2/2/5 | 3230582 / 3167379 | 21342 | 6211 | 3472 | 20968 | 24440 | — | 0 |
| 03-WF | 31 | 34/2/0/0 | 0 / 0 | 20445 | 5221 | 16566 | 2461 | 19027 | 61622 | 18 |
| 04-SF | 31 | 0/0/2/2 | 3255134 / 3187141 | 25925 | 6496 | 13936 | 2320 | 16256 | 67993 | 1 |
| 05-SE | 3 | 0/2/2/5 | 3231456 / 3167942 | 36337 | 6595 | 3561 | 17021 | 20582 | — | 0 |
| 06-SB | 7 | 0/2/2/5 | 3269273 / 3203055 | 30467 | 11893 | — | — | 19472 | — | 0 |
| 07-SB | 31 | 0/0/2/2 | 3234171 / 3171537 | 24985 | 5653 | — | — | 15659 | 62634 | 1 |
| 08-SE | 31 | 10/2/0/0 | 0 / 0 | 19044 | 5463 | 2836 | 13495 | 16331 | 57230 | 8 |
| 09-SF | 31 | 10/2/0/0 | 0 / 0 | 18389 | 8248 | 17472 | 2652 | 20124 | 61194 | 8 |
| 10-WF | 31 | 34/2/0/0 | 0 / 0 | 22905 | 5505 | 14689 | 2393 | 17082 | 61876 | 18 |
| 11-WE | 31 | 34/2/0/0 | 0 / 0 | 20302 | 6428 | 3422 | 16723 | 20145 | 61127 | 18 |
| 12-WB | 31 | 0/0/2/2 | 3219306 / 3154576 | 23536 | 6653 | — | — | 19441 | 64730 | 1 |

## Repeat comparisons

- W/E: prefix 3472/24440 (14.2%), suffix 20968/24440 (85.8%), signed prefix minus suffix -17496, prefix 3422/20145 (17.0%), suffix 16723/20145 (83.0%), signed prefix minus suffix -13301. Same side larger in both: True. B whole [29711, 19441], diagnostic whole [24440, 20145]. Transfer to B supported by range and reach checks: False.
- W/F: prefix 16566/19027 (87.1%), suffix 2461/19027 (12.9%), signed prefix minus suffix +14105, prefix 14689/17082 (86.0%), suffix 2393/17082 (14.0%), signed prefix minus suffix +12296. Same side larger in both: True. B whole [29711, 19441], diagnostic whole [19027, 17082]. Transfer to B supported by range and reach checks: False.
- S/E: prefix 3561/20582 (17.3%), suffix 17021/20582 (82.7%), signed prefix minus suffix -13460, prefix 2836/16331 (17.4%), suffix 13495/16331 (82.6%), signed prefix minus suffix -10659. Same side larger in both: True. B whole [19472, 15659], diagnostic whole [20582, 16331]. Transfer to B supported by range and reach checks: False.
- S/F: prefix 13936/16256 (85.7%), suffix 2320/16256 (14.3%), signed prefix minus suffix +11616, prefix 17472/20124 (86.8%), suffix 2652/20124 (13.2%), signed prefix minus suffix +14820. Same side larger in both: True. B whole [19472, 15659], diagnostic whole [16256, 20124]. Transfer to B supported by range and reach checks: False.

No cross-boot subtraction or imputed counter values. These results only identify potential regions for further source investigation. High-water stack depth and physical-board timing are unmeasured.

## Linked footprint

Bytes by image (text/data/bss).

~~~
text	   data	    bss	    dec	    hex	filename
  24862	   1800	 132888	 159550	  26f3e	candidate-SB.elf
  25022	   1800	 132912	 159734	  26ff6	candidate-SE.elf
  25010	   1800	 132912	 159722	  26fea	candidate-SF.elf
  24858	   1800	 132888	 159546	  26f3a	candidate-WB.elf
  25014	   1800	 132912	 159726	  26fee	candidate-WE.elf
  25002	   1800	 132912	 159714	  26fe2	candidate-WF.elf
~~~

The public ZIP substitutes `capture-user` for the account component of absolute home paths in 22 text members. The original archive is retained privately. The JSON records original and public sizes and SHA-256 digests for each member and both archives. The public freeze and command metadata retain the recorded argument order and numeric values. The guest streams, ELFs and disassemblies are byte-identical.

```sh
python3 -m zipfile -e reservation-stage-v3-first.zip v3
python3 -c "import hashlib,json,pathlib; p=pathlib.Path('v3'); j=json.loads(pathlib.Path('reservation-stage-v3-first.json').read_text()); assert all(hashlib.sha256((p/n).read_bytes()).hexdigest()==h for n,h in j['public_member_sha256'].items())"
```
