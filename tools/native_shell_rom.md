# Package native shell executables and input files

Set SHELL_ELF, CAT_ELF and WC_ELF to three independently built native-v1
AArch64 executable files. Run from the repository root. The following example
creates an input file with three newline characters and packages its exact bytes
under the guest name sample.txt. Choose an unused output directory.

~~~sh
mkdir shell-input
printf '%s\n' rain wind light > shell-input/sample.txt
python3 tools/build_native_el0.py --board virt --pipes --policy fifo --out shell-fifo \
    --elf "$SHELL_ELF" --priority 3 --peer-elf "$CAT_ELF" --peer-priority 3 \
    --peer-elf "$WC_ELF" --peer-priority 3 --exec-child 1:cat --exec-child 2:wc \
    --rom-file sample.txt=shell-input/sample.txt
~~~

For RR change the output directory and use `--policy rr --quantum-ticks 625000`.
The shell occupies slot 0 with executable name sh. Each of slots 1 and 2
requires one distinct child name. Use executable and guest file basenames of
1–31 ASCII letters, digits, dots, dashes or underscores. A slash, "." and ".."
are invalid. The host source path follows the first equals sign and may contain
directories or further equals signs. File and executable names occupy separate
namespaces. The shell runtime owns executable launch and descriptor wiring.

The manifest records exec_names in slot order, the SHA-256 of native_shell.rom,
and each guest file's name, offset, exact length and SHA-256. Up to eight entries
share 7792 data bytes. An empty input file has length zero. Omitting all
--rom-file options produces an empty, valid ROM. Entries are sorted by guest
basename. The 8192-byte image has magic JNOXROM1, little-endian u32 count at
offset 8 and eight 48-byte entry slots starting at 16. An entry contains a
32-byte NUL-padded name, u32 offset and u32 length. Data starts at 400. All
reserved bytes and remaining capacity are zero.

For a direct native Cargo build, retain the original native image, peer ELF
hashes, fixed policy and pipe environment. Add these variables, pointing to a
durable copy of the ROM and using its independently recorded manifest digest:

~~~sh
export JNOX_NATIVE_EXEC_CHILDREN='1:cat,2:wc'
export JNOX_NATIVE_ROM_IMAGE="$PWD/shell-fifo/native_shell.rom"
export JNOX_NATIVE_ROM_SHA256=$(sha256sum "$JNOX_NATIVE_ROM_IMAGE" | cut -d' ' -f1)
~~~

Compare that digest with the intended manifest before invoking Cargo. The build
snapshots and hashes the resource, validates every table entry and exposes the
verified absolute snapshot path to the kernel. A successful package establishes
input identity. Guest evidence must separately show the native shell entering
`cat sample.txt | wc -l`, opening the file, launching both named children,
connecting their descriptors and collecting their real output and exits.
