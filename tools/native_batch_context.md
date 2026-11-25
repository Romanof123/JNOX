# Batch progress and integer context

Use the retained 25a4198 verification archive's `products` directory, its unmodified binary packages and their original captures. From the supplemental source root, with the matching QEMU 6.2 stock binary on PATH and its plugin header installed:

~~~sh
python3 tools/native_batch_context.py historical "$PRODUCTS" > original-context.json
cc -std=c11 -O2 -fPIC -shared -I"$QEMU_PLUGIN_INCLUDE" tools/native_batch_context.c -o context.so
PLUGIN_SHA=$(sha256sum context.so | cut -d' ' -f1)
python3 tools/native_batch_context.py acquire "$PRODUCTS" new-observed context.so "$PLUGIN_SHA"
python3 tools/native_batch_context.py observed "$PRODUCTS" new-observed arrival-srtf "$SRTF_SESSION_SHA" "$PLUGIN_SHA" > observed-srtf.json
python3 tools/native_batch_context.py observed "$PRODUCTS" new-observed tie-sjf "$SJF_SESSION_SHA" "$PLUGIN_SHA" > observed-sjf.json
~~~

Use the two session digests printed by acquisition. One acquisition invocation creates a fresh directory and launches SRTF then SJF once each. It retains the QEMU argv, plugin ELF, shared backing file identity and bytes, bounded plugin stream, exceptions, UART and terminal snapshot for each attempt. The 128 MiB file-backed memory object changes host backing and callback overhead. Timing conclusions apply only to these observer boots. Keep the first results, including incomplete or coalesced runs.

Historical JSON contains x0 through x30 in register order and reports the completed progress stores inferred from W21 separately from the terminal private values. The historical plain and traced counts are 13,721 and 13,619. Neither first has an independent transient RAM read or actual restore operand capture. Observed JSON retains each raw operand array including the 31 integer words plus ESR, SPSR, ELR and user SP. A matching restore identifies the operand consumed by the actual kernel load. A coalesced SJF release has its initial store count and subsequent publication and exit order recorded independently of the missing incumbent-retention case. Replay exit 2 identifies missing observations, exit 1 a failed integrity check or incomplete session, and exit 0 completion of the requested observations.
