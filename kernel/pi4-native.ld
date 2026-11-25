OUTPUT_ARCH(aarch64)
ENTRY(_start)

PHDRS {
    stub PT_LOAD FLAGS(5);
    code PT_LOAD FLAGS(5);
    writable PT_LOAD FLAGS(6);
}

SECTIONS {
    .shim 0x00080000 : {
        KEEP(*(.text.pi4_native_shim))
        KEEP(*(.pi4_native_secondary_shim))
    } :stub

    __native_payload_lma = 0x00081000;
    __native_payload_start = 0x02000000;
    .text __native_payload_start : AT(__native_payload_lma) {
        KEEP(*(.text.boot))
        *(.text .text.*)
    } :code
    .rodata ALIGN(16) : AT(__native_payload_lma + ADDR(.rodata) - __native_payload_start) {
        *(.rodata .rodata.* .srodata .srodata.*)
    } :code
    .data ALIGN(4096) : AT(__native_payload_lma + ADDR(.data) - __native_payload_start) {
        __native_rw_start = .;
        *(.data .data.* .sdata .sdata.* .got .got.*)
    } :writable
    __native_payload_copy_end = SIZEOF(.data) ? ADDR(.data) + SIZEOF(.data) : ADDR(.rodata) + SIZEOF(.rodata);
    __native_payload_size = __native_payload_copy_end - __native_payload_start;
    __native_payload_lma_end = __native_payload_lma + __native_payload_size;

    .bss (NOLOAD) : ALIGN(4096) {
        __bss_start = .;
        *(.bss .bss.* .sbss .sbss.* COMMON)
        . = ALIGN(8);
        __bss_end = .;
    } :writable
    .stack (NOLOAD) : ALIGN(16) {
        __native_kernel_stack_bottom = .;
        . += 16K;
        __stack_top = .;
    } :writable
    /DISCARD/ : { *(.eh_frame .eh_frame_hdr .note.gnu.build-id) }
}
ASSERT(_start == 0x80000 && ADDR(.shim) + SIZEOF(.shim) <= __native_payload_lma, "Pi raw stub exceeds its first page")
ASSERT(pi4_native_payload_entry == __native_payload_start, "relocated startup must be first")
ASSERT(__native_payload_lma == LOADADDR(.text), "payload raw source mismatch")
ASSERT(__native_payload_copy_end > __native_payload_start && __native_payload_copy_end <= __bss_start, "invalid copy destination")
ASSERT(__native_payload_lma_end > __native_payload_lma && __native_payload_lma_end <= __native_payload_start, "invalid or overlapping copy source")
ASSERT(__native_payload_start >= 0x01011000 && (__native_rw_start & 4095) == 0, "kernel must be separate from EL0 stack and image")
ASSERT(__bss_start <= __bss_end && __bss_end <= __native_kernel_stack_bottom && (__stack_top & 15) == 0, "invalid BSS or stack")
ASSERT(__stack_top <= 0x04000000, "live Pi kernel exceeds native identity mapping")
