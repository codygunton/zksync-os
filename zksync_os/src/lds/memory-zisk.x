/*
 * Memory layout for Zisk zkVM
 *
 * Zisk expects:
 *   ROM at 0x80000000 - program code and read-only data
 *   RAM at 0xa0000000 - stack, heap, BSS, mutable data
 *
 * This matches Zisk's memory map defined in zisk/core/src/mem.rs:
 *   ROM_ADDR = 0x80000000
 *   RAM_ADDR = 0xa0000000
 *
 * Used by: config.toml (default), setup_zksyncos.sh, execute.sh
 */

MEMORY
{
  ROM (rx): ORIGIN = 0x80000000, LENGTH = 128M
  RAM (rwa!x) : ORIGIN = 0xa0000000, LENGTH = 512M
}

REGION_ALIAS("REGION_TEXT", ROM);
REGION_ALIAS("REGION_DATAINIT", ROM);
REGION_ALIAS("REGION_RODATAINIT", ROM);
REGION_ALIAS("REGION_STACK", RAM);
REGION_ALIAS("REGION_DATA", RAM);
REGION_ALIAS("REGION_RODATA", ROM);
REGION_ALIAS("REGION_BSS", RAM);
REGION_ALIAS("REGION_HEAP", RAM);
