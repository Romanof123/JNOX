OUTPUT_FORMAT("elf64-littleaarch64")
OUTPUT_ARCH(aarch64)
ENTRY(_start)
PHDRS
{
    text PT_LOAD FLAGS(5);
    ro   PT_LOAD FLAGS(4);
    rw   PT_LOAD FLAGS(6);
}
SECTIONS
{
    . = 0x00400000;
    .text : { KEEP(*(.text.entry)) *(.text .text.*) } :text
    . = ALIGN(4096);
    .note.jnox.abi : { KEEP(*(.note.jnox.abi)) } :ro
    .rodata : { *(.rodata .rodata.*) } :ro
    . = ALIGN(4096);
    .data : { *(.data.private_report) *(.data .data.*) } :rw
    .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } :rw
    ASSERT(_start == 0x00400000, "entry must be first executable byte")
    ASSERT(. <= 0x00800000, "image exceeds the native user window")
    /DISCARD/ : { *(.comment) *(.note.GNU-stack) *(.llvm_addrsig) }
}
