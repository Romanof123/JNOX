# EVD-054 · Targeted phase and proportional comparison

[JSON](evd-054-targeted.json) · [ZIP](evd-054-targeted.zip) · [#131](https://github.com/Romanof123/JNOX/pull/131) · [#136](https://github.com/Romanof123/JNOX/pull/136) · [#142](https://github.com/Romanof123/JNOX/pull/142) · [Source](https://github.com/Romanof123/JNOX/tree/3b5a9f69034ecc2b7a7b093ddb786ee74f820be6)

All 21 admitted release images had identical loader inputs to their integrated references. Each whole ELF hash differed. All 44 negative configurations produced a selector rejection. The 65 builds added zero guest boots.

Ten phase images map to 32 earlier guest positions. Eight control images map to eight historical boots and two separate mode-3 probe captures. Three proportional images map to lottery1, lottery2 and stride with 32, 32 and 16 grants. The archive associates historical run IDs and raw hashes with each reference. Earlier healthy-checker failures and admission limits still apply.

The ELF comparisons covered entry point, segment flags, addresses, file and memory sizes, alignment, initialized bytes and zero fill. Both linked interfaces retained vector base, restore entry, actual ERET address, stack bounds, 16-byte alignment, 16-KiB task stacks and 832-byte frame allocation/restoration. The strict graph recorded zero findings and six graph tests passed. The gate checked the first and integrated evidence archives for both policies. It supplies no new timing or operational approval.

For the #142 host-tool documentation bridge, a separate literal search inspected every archived .py/.sh member. EVD-054’s integrated nested tar contains 14 scripts including run.py. The #136 first and integrated ZIPs contain three and four respectively, including launch.py in each. Neither qemu_boot nor fixed_runtime_observer appeared in these scripts or the EVD-054 first capture scripts. The JSON lists paths, hashes and the search scope. Indirect dependencies were outside that check.

```sh
unzip evd-054-targeted.zip -d targeted
(cd targeted && sha256sum -c SHA256SUMS)
```
