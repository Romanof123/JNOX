OUTPUT_ARCH(aarch64)
ENTRY(_start)

/* The 128 MiB bench starts RAM at 0x40000000. Leave its first 2 MiB
   to QEMU for the bare-metal device tree. VMA equals load address. */
MEMORY {
    ram (rwx) : ORIGIN = 0x40200000, LENGTH = 126M
}
PHDRS {
    code PT_LOAD FLAGS(5);
    writable PT_LOAD FLAGS(6);
}
SECTIONS {
    .text : {
        KEEP(*(.text.boot))
        *(.text .text.*)
    } >ram :code
    .rodata : { *(.rodata .rodata.*) } >ram :code
    .data : ALIGN(4096) { __native_rw_start = .; *(.data .data.*) } >ram :writable
    .bss (NOLOAD) : ALIGN(8) {
        __bss_start = .;
        *(.bss .bss.* COMMON)
        . = ALIGN(8);
        __bss_end = .;
    } >ram :writable
    .stack (NOLOAD) : ALIGN(16) {
        . += 16K;
        __stack_top = .;
    } >ram :writable
    /DISCARD/ : { *(.eh_frame .eh_frame_hdr) }
}
