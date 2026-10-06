/* OASIS flash layout under the A/B bootloader (docs/specs/FIRMWARE_UPDATE_SPEC.md §2).
   The application's memory_bootloaded.x must use the same partitions. OASIS state
   (0x1F2000..0x1FFFFF: anti-rollback floor, registry, owner, identity, policy,
   revocation, lease, v0B window) lies above DFU and is never touched here. */
MEMORY
{
  BOOT2            : ORIGIN = 0x10000000, LENGTH = 0x100
  FLASH            : ORIGIN = 0x10000100, LENGTH = 28K - 0x100
  BOOTLOADER_STATE : ORIGIN = 0x10007000, LENGTH = 4K
  ACTIVE           : ORIGIN = 0x10008000, LENGTH = 512K
  DFU              : ORIGIN = 0x10088000, LENGTH = 516K
  RAM              : ORIGIN = 0x20000000, LENGTH = 264K
}

__bootloader_state_start = ORIGIN(BOOTLOADER_STATE) - ORIGIN(BOOT2);
__bootloader_state_end = ORIGIN(BOOTLOADER_STATE) + LENGTH(BOOTLOADER_STATE) - ORIGIN(BOOT2);

__bootloader_active_start = ORIGIN(ACTIVE) - ORIGIN(BOOT2);
__bootloader_active_end = ORIGIN(ACTIVE) + LENGTH(ACTIVE) - ORIGIN(BOOT2);

__bootloader_dfu_start = ORIGIN(DFU) - ORIGIN(BOOT2);
__bootloader_dfu_end = ORIGIN(DFU) + LENGTH(DFU) - ORIGIN(BOOT2);
