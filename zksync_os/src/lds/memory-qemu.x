/*
 * Memory layout for QEMU virt machine debugging
 *
 * QEMU -kernel always jumps to 0x80000000, so ROM must be there.
 * RAM is placed at 0x88000000 (right after 128M ROM region).
 *
 * Note: Debug symbols (debuginfo=2) work but GDB may show warnings about
 * DWARF relocations for addresses >= 0x80000000. Symbols still function.
 *
 * Memory map:
 *   ROM at 0x80000000 (128M) - program code and read-only data
 *   RAM at 0x88000000 (512M) - stack, heap, BSS, mutable data
 *
 * Used by: build.sh qemu, debug_qemu.sh
 */

MEMORY
{
  ROM (rx): ORIGIN = 0x80000000, LENGTH = 128M
  RAM (rwa!x) : ORIGIN = 0x88000000, LENGTH = 512M
}

REGION_ALIAS("REGION_TEXT", ROM);
REGION_ALIAS("REGION_DATAINIT", ROM);
REGION_ALIAS("REGION_RODATAINIT", ROM);
REGION_ALIAS("REGION_STACK", RAM);
REGION_ALIAS("REGION_DATA", RAM);
REGION_ALIAS("REGION_RODATA", ROM);
REGION_ALIAS("REGION_BSS", RAM);
REGION_ALIAS("REGION_HEAP", RAM);
