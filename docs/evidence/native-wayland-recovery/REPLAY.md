# Reproduce the #449 offline review

Keep jnox449-evidence.zip and the extracted review kit together. Use Linux or WSL with Python 3.10+ and GNU binutils nm supporting AArch64 ELF. Run from that directory with fresh restored and replay-output paths. Set PYTHONDONTWRITEBYTECODE to keep the restored tree byte-for-byte unchanged.

```sh
echo '98b001ff9f7e38ee7b869df9aaa3fd2f3e7cc8917ae5b6112b74a0f396ed1cf8  jnox449-evidence.zip' | sha256sum -c -
python3 evidence.py restore jnox449-evidence.zip restored
export PYTHONDONTWRITEBYTECODE=1
python3 restored/independent/review_frozen.py restored/449-build-02 replay-output
python3 regressions.py restored
cmp replay-output/summary.json restored/independent/final-review/summary.json
```

The restore checks every file SHA-256 and the expanded memory of each .ram.gz capture. The manifest in the canonical archive provides every original filename, file size, timestamp, mode and content hash. Review_frozen runs all three corrected scenario replays, exercises the three mutation controls for each applicable recovery case and recalculates the full timing traces. Regressions rechecks window screenshots against their recorded baseline pixels, headless UART and every CSV row directly against the four terminal memory captures. Python assertions must be enabled. Neither driver starts a guest.

The archive contains 394 files and 50,526,613 restored file bytes, including 30 individually checksummed original gzip RAM files. Together they decode to 4,026,531,840 bytes. Raw captures from the initial failure sequence are under 449-build-01. Corrected captures, role packages, both regressions and all timing rows are under 449-build-02. Historical replay output and original mutation scripts are under independent. Negative tests deliberately modify in-memory copies.

The corrected restart and fault recovery intervals had no gap above 20 ms. Over the entire run, restart had one 49.418464 ms gap at sample 4 and three releases at least one period late. Fault had one 468.369296 ms gap at sample 10 and 45 releases at least one period late. Both gaps preceded the first primary frame. Primary exit and headless each had zero threshold violations.
