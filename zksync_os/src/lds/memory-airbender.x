/*
 * Memory layout for airbender zkVM (original upstream layout)
 *
 * This matches the original memory.x used by airbender.
 * ROM at 0, RAM at 2M with 1022M available.
 */

MEMORY
{
  ROM (rx): ORIGIN = 0, LENGTH = 2M
  RAM (rwa!x) : ORIGIN = 2M, LENGTH = 1022M
}

REGION_ALIAS("REGION_TEXT", ROM);
REGION_ALIAS("REGION_DATAINIT", ROM);
REGION_ALIAS("REGION_RODATAINIT", ROM);
REGION_ALIAS("REGION_STACK", RAM);
REGION_ALIAS("REGION_DATA", RAM);
REGION_ALIAS("REGION_RODATA", RAM);
REGION_ALIAS("REGION_BSS", RAM);
REGION_ALIAS("REGION_HEAP", RAM);
