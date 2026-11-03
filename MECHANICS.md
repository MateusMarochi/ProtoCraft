# Project mechanics and source rules

This file records the implemented compatibility rules for ProtoCraft and the source behavior found in the installed Proteus project manager.

## Source inspected

- Program: `C:/Program Files (x86)/Labcenter Electronics/Proteus 8 Professional/BIN/PDS.EXE`
- SHA-256: `e6adc25a6cfaef2aa021a6aa24fd63311302343d20e2246b2c4c77422d47764f`
- The addresses below are PDS.EXE function entry addresses. The pseudocode was produced in Deep mode by Ghidra; it is decompiler output, not vendor source.

## Undo history

- `FUN_00408510` at `0x408510` reads the registry value `Undo Levels` with default `0x14` (decimal `20`).
- PDS treats this as a user setting, so 20 is its default, not an immutable maximum.
- ProtoCraft has no matching setting control. Its edit history uses this default as a fixed cap: after an edit snapshot is pushed, discard the oldest snapshot while the count exceeds 20.
- Implementation: `eda-core::MAX_UNDO_LEVELS = 20` in `crates/eda-core/src/lib.rs`.

## Project backup generations

- `FUN_00408510` at `0x408510` reads `Backup Levels` with default `10`. It initializes `Backup Folder` to `Project Backups` before reading a configured override.
- `PDSPROJECT::backup` at `0x41B9A0` uses the project stem and the configured backup folder. For its timestamped-project branch, the filename is `<folder>/<stem> [<yyyyMMdd>, <HH-mm-ss>].pdsprj`; the date and time come from the project's `MODIFIED` timestamp. Without a backup folder, the fallback name is `Backup of <stem>.pdsbak`.
- When a backup folder is set, PDS searches matching timestamped project copies, orders them by the two timestamp fields, then unlinks entries starting at index `Backup Levels`. The configured count is the number retained.
- ProtoCraft maps the default count to ten sidecar generations: `<project>.bak` is the file that immediately preceded the latest save; `<project>.bak.1` through `<project>.bak.9` are successively older files. On each save, shift generations from 9 down to 1, discard anything older than generation 9, write the prior valid project to `.bak`, then atomically write the new project. Corrupt or unsupported current projects are rejected before rotation.
- Implementation: `eda-storage::MAX_BACKUP_GENERATIONS = 10` and `rotate_backups` in `crates/eda-storage/src/lib.rs`. This preserves PDS's default retention count while using ProtoCraft sidecar names instead of PDS's timestamped folder layout.

## Project naming, extension, and validation

- `FUN_0040CB20` at `0x40CB20` and `FUN_0040EAA0` at `0x40EAA0` handle a selected path. If it is a directory, PDS derives the name from the directory's `dirName`; if it is a file, it uses the file's `baseName`. The project folder is the selected directory or the selected file's absolute parent.
- Both handlers call `FUN_0040EF10` at `0x40EF10`, which appends the suffix it receives. The project-creation validator at `FUN_0040D940` (`0x40D940`) first checks whether the name already ends in `.pdsprj` (case-insensitive) and appends the suffix only when needed.
- ProtoCraft's `protocraft-cli new --output <path>` follows the same missing-suffix rule for its format: when the path has no extension, append `.protocraft`; accept an explicit `.protocraft` extension case-insensitively and reject another extension. The inferred project name is the resulting file stem; `--name` overrides it.
- PDS validation requires a nonempty project name, a nonempty folder, and a folder that exists. Its messages include `You must give a project name`, `You must set a project folder`, and `Invalid Project Folder`.
- If the destination project already exists, PDS asks `Replace existing project?` with the title `Project Already Exists`; the user can cancel. The wizard has a related path in `FUN_0042DDC0` at `0x42DDC0`.
- ProtoCraft's CLI `new` refuses an existing destination to preserve its contents. Existing-project saves keep the valid previous file through the ten-generation backup rule above; ProtoCraft does not yet reproduce PDS's replacement dialog.

## Autosave setting found, feature not implemented

- `FUN_00408510` at `0x408510` reads `Autosave Interval` with default `10` minutes. `FUN_00415710` at `0x415710` writes an `[Autosaved]` copy, using `.pdsbak` without a backup folder or an `[Autosaved].pdsprj` path within the configured folder.
- ProtoCraft has no background autosave counterpart. The ten-minute value is recorded for source accuracy but is not applied to ProtoCraft's save behavior.
