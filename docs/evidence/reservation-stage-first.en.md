# CHG-071 · First reservation stage preflight

[CHG-071](../reservation-stage-attribution-plan.md) · [#153](https://github.com/Romanof123/JNOX/pull/153) · [Parent](https://github.com/Romanof123/JNOX/tree/ff9bf821d280fead823b3edddd0a20ff21c9f6f0) · [Candidate](https://github.com/Romanof123/JNOX/tree/2b210259679ae9271852f221ba7ccbeae86c9088) · [JSON](reservation-stage-first.json) · [ZIP](reservation-stage-first.zip)

The aligned-wake baseline failed the required load-image identity check. Zero of the twelve planned guests launched.

The separately built parent ELF reproduced the archived size-optimized aligned-wake ELF. The candidate and parent had identical 23,864-byte .text sections. Both PT_LOAD headers agreed in flags, addresses, sizes and alignment. Their 24,858-byte executable segment differed at ten bytes in ten Rust source-location line words within the 994-byte .rodata section. The JSON lists each address and both little-endian values. The writable segment matched throughout its initialized bytes and zero-fill extent.

This ends the first preflight at W/B. The CBS saturated baseline and the E (post-EOI) and F (second frame validated) images were untested. No triplet, guest timing, selected worker, physical measurement or dynamic cost attribution followed. Both verbose build logs, ELFs, extracted sections, disassemblies, symbol maps and original comparison scripts are archived. The full source archive hashes remain in SHA256SUMS. Recover those sources from the two pinned commits. Use INCLUDED.sha256 for the published subset.

```sh
unzip reservation-stage-first.zip -d first-stage
(cd first-stage && sha256sum -c INCLUDED.sha256)
```
