/* Application under oasis-bootloader (feature `bootloaded`), Phase 1.3.
   Same partitions as oasis-bootloader/memory.x: this image runs from ACTIVE at
   0x10008000; the bootloader owns boot2 and 0x10000000..0x10007FFF. */
MEMORY
{
    FLASH : ORIGIN = 0x10008000, LENGTH = 512K
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K
}

/* Version header right after the 48-word vector table (offset 0xC0), covered by
   the image hash: "OFWI" | version u32 LE (oasis_rt::firmware::FWINFO_OFFSET).
   .text is moved to 0x100 so it cannot overlap the header. */
_stext = ORIGIN(FLASH) + 0x100;

SECTIONS {
    .oasis_fwinfo ORIGIN(FLASH) + 0xC0 :
    {
        KEEP(*(.oasis_fwinfo));
    } > FLASH
} INSERT AFTER .vector_table;
