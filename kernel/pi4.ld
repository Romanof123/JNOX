OUTPUT_ARCH(aarch64)
ENTRY(_start)

/* The board config pins the raw image load address. Bound this serial-only
   payload to a small low-RAM window, including its NOLOAD BSS and stack. */
MEMORY {
    ram (rwx) : ORIGIN = 0x80000, LENGTH = 512K
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
    .data : ALIGN(4096) { *(.data .data.*) } >ram :writable
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
ASSERT(_start == ORIGIN(ram), "raw image must start at the firmware entry")
