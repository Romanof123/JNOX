# OFF observer cost · Aborted first cohort

[#144](https://github.com/Romanof123/JNOX/pull/144) · [CHG-061](../permit-observer-cost-plan.md) · [CLI](https://github.com/Romanof123/JNOX/blob/dc9f25897e661dae84be09d91c92dd713743d3a0/tools/permit_observer_cost.py) · [JSON](permit-observer-first-partial.json) · [ZIP](permit-observer-first-partial.zip)

One of 16 frozen positions launched. The other 15 never ran. No pair among eight produced an R−F difference.

Position 01-rm-off-1-F reached a UART terminal marker. The collector classified the 69-byte QEMU SIGTERM diagnostic as unexpected stderr and recorded the position as incomplete. QEMU process status was 0. The pinned collector stops on this disposition, writes cohort.json with the unlaunched positions and returns 1 for the incomplete cohort. The archive retains first-seen, terminal, alarm and stop host timestamps and all eight null pair records.

The intact UART yields 13,171 ticks of first IRQ lateness, 55,167 ticks of first phase delay and 20,707 ticks of first service. It records reason 103 and two write ordinals with zero before/after samples. Six host vectors passed and the separate synthetic child control detected an asserted bit and stopped the child. Those controls do not establish the full guest output history.

Only the first 25 and last 10 lines survive from the 75,321-byte mechanism stream. The original hash comes from the cohort metadata. The 2,302-byte excerpt has its own digest. The freeze digest was printed, but the freeze file is missing. Exhaustive model events, correspondence to a guest tick, observation cost and physical behavior remain undetermined.

| File | Bytes | SHA-256 |
|---|---:|---|
| UART | 884 | ee5c5eef236d452dc545c37465affe91f372d23da5fc2be5f965a6af2b3706eb |
| stderr | 69 | bd17ff5d7d1a2061850aea97cbd32fd1ad7ae1747858792cc9fb7a7029c0530d |
| mechanism (original) | 75321 | 23ecdf8590a9a0a137e59fa9b1b705ef44a7b139334411d4f7d02bd26f9823db |
| mechanism (head 25 + tail 10) | 2302 | 528be28e0c7fd657987fb8e5fa37a2bb4acb98f4a0078b42cced11c6e9c6b097 |

Proposed retention change: persist the frozen inputs and full raw streams and distinguish the expected supervisor termination diagnostic from other stderr. A future cohort needs its own denominator. [#144](https://github.com/Romanof123/JNOX/pull/144)

```sh
unzip permit-observer-first-partial.zip -d first-partial
(cd first-partial && sha256sum -c SHA256SUMS)
```
