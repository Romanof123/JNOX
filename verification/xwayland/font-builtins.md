# Xwayland core bitmap fonts for the JNOX target

Xwayland 24.1.6 needs the xfont2 server-side library. The initial target profile selects its upstream built-in font path element `built-ins`. Its `fixed` and `6x13` aliases name the real ISO-8859-1 PCF from the release; the same source contains the cursor font. Keep the release's SuSE copyright and permission notice with redistributions. The prototype can use Xwayland's `-fp built-ins` without mounting a font directory. UTF-8 text shaping, scalable fonts, external encoding tables and Xft/Fontconfig require separate application support.

| Input | Role |
| --- | --- |
| [X.Org libXfont2 2.0.7 archive](https://xorg.freedesktop.org/archive/individual/lib/libXfont2-2.0.7.tar.xz) | `src/builtins/fonts.c` supplies original gzip-compressed PCF assets. `file.c` opens and inflates their streams and `render.c` calls the PCF bitmap reader. `src/builtins/fpe.c` registers the named font path. |
| [X.Org libfontenc 1.1.8 archive](https://xorg.freedesktop.org/archive/individual/lib/libfontenc-1.1.8.tar.xz) | Required target encoding library. Built with xproto and zlib. |
| [zlib 1.3.1 archive](https://zlib.net/fossils/zlib-1.3.1.tar.gz) | Uncompresses the embedded font. |
| Existing #448 target protocol prefix | xproto, fontsproto ≥2.1.3 and xtrans; use the staged 2024.1 headers and 1.4.0 transport. |
| JNOX target C toolchain | Static libc/libm, including heap, I/O, and `hypot`. The complete configured archive can reference file APIs even when only built-ins are selected at runtime. The diagnostic configuration in `configure.sh` needs these additional runtime facilities. |

The source tarballs include configure scripts and generated tables. No autoreconf or host font conversion is required. The target build explicitly enables built-ins and PCF, disables FreeType, BDF, SNF, and the network font-server client, and omits bzip2. It uses the target prefix exclusively for pkg-config. Host tools are a POSIX shell, tar with gzip/xz support, make, pkg-config, sha256sum, readelf, LLVM archive tools and the JNOX cross compiler/linker. Use fresh work and dedicated prefix directories without spaces; first populate the prefix with the intake's protocol headers/metadata. Use a compiler wrapper that supplies the gated JNOX C runtime, startup, target include path, linker script and native syscall ABI for **all** configure link probes and the final target probe.

```sh
TARGET_CC=/absolute/path/to/jnox-cc sh verification/xwayland/font-static.sh "$CACHE" "$NEW_WORK" "$TARGET_PREFIX"
```

The script verifies each release archive by SHA-256, retains configuration attempts and checks the architecture of every static archive member. Its final artifact is `font-builtins-probe.elf`. The integration gate must run this as a JNOX native program, retain the real exit status and read the exported `jnox_font_probe_rows` from the process address space. The probe calls upstream `BuiltinFileOpen("6x13.builtin")` and reads the decompressed font through libXfont2 and zlib. It checks the length, PCF magic/table formats, encoding for U+0041 and thirteen actual bitmap rows. Failure codes 1–7 distinguish open, size/format, table, encoding range, glyph index, bitmap bounds and pixel mismatch.

The source-derived reference for the 19,584-byte uncompressed PCF has SHA-256 `6f1bc71fb2fd55b70b07a6d59df6ee5a2bb8bd3e052b6f08925db54fed65b339`. U+0041 maps to glyph index 65. Pixel data starts at byte 6,288 with four-byte row padding. Expect native process exit 0 and the following bytes in its exported row buffer:

```text
00 00 04 0a 11 11 11 1f 11 11 11 00 00
```

A subsequent full-server gate must open `fixed` over the real X11 connection, draw U+0041 and compare target image pixels with this reference. A successful library probe establishes the embedded-file, decompressor and glyph-data route only. Compilation, native execution and server rendering await the serialized integration gate.
