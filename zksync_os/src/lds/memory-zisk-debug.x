/*
 * Debug memory layout for Zisk zkVM
 *
 * Uses LOW addresses to avoid 32-bit relocation overflow in debug sections.
 * Code is identical to production build, just relocated.
 *
 * To map addresses from production build to debug build:
 *   debug_addr = prod_addr - 0x80000000 + 0x00100000
 *
 * Example: 0x8006e388 -> 0x0016e388
 */

MEMORY
{
  ROM (rx): ORIGIN = 0x00100000, LENGTH = 128M
  RAM (rwa!x) : ORIGIN = 0x10000000, LENGTH = 512M
}

REGION_ALIAS("REGION_TEXT", ROM);
REGION_ALIAS("REGION_DATAINIT", ROM);
REGION_ALIAS("REGION_RODATAINIT", ROM);
REGION_ALIAS("REGION_STACK", RAM);
REGION_ALIAS("REGION_DATA", RAM);
REGION_ALIAS("REGION_RODATA", ROM);
REGION_ALIAS("REGION_BSS", RAM);
REGION_ALIAS("REGION_HEAP", RAM);
