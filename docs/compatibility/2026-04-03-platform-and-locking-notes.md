# Platform and Locking Compatibility Notes

## Overview

This document describes platform-specific behaviors and file locking considerations for `codex-doctor`.

## File Locking

### SQLite Database Locking

**Problem:** SQLite databases (`state_5.sqlite`, `logs_2.sqlite`) use file-based locking. If Codex or another process has the database open, repair operations will fail.

**Behavior by Platform:**

#### Windows
- SQLite uses mandatory file locking
- Attempting to open a locked database returns `SQLITE_BUSY`
- `codex-doctor` detects this and marks the operation as `skipped` with `retryable: true`
- **Recommendation:** Close all Codex processes before running repairs

#### macOS/Linux
- SQLite uses advisory file locking (fcntl)
- Multiple readers are allowed, but writers block
- Same detection and retry behavior as Windows
- **Recommendation:** Close all Codex processes before running repairs

### Rollout File Locking

**Problem:** Rollout files (`.jsonl` files in `sessions/` and `archived_sessions/`) may be locked during active writes.

**Behavior:**
- Windows: Exclusive locks prevent reads during writes
- macOS/Linux: Advisory locks, but Codex typically uses exclusive write mode
- `codex-doctor` detects locked files and skips them with retry metadata

**Mitigation:**
- Run repairs when Codex is idle
- Use `--dry-run` first to identify locked resources
- Retry after closing active sessions

## Backup Performance

### Same-Filesystem Optimization

**Best Practice:** Place backup directory on the same filesystem as `.codex`

**Reason:**
- Same-filesystem copies can use reflinks (on supported filesystems)
- Cross-filesystem copies require full data transfer
- Large SQLite databases (100MB+) benefit significantly

**Supported Filesystems:**
- **Linux:** btrfs, XFS (with reflink support)
- **macOS:** APFS (automatic copy-on-write)
- **Windows:** ReFS (block cloning), NTFS (no reflink, but fast local copy)

### Backup Size Considerations

Typical `.codex` directory sizes:
- Small (< 10 sessions): 10-50 MB
- Medium (10-100 sessions): 50-500 MB
- Large (100+ sessions): 500 MB - 2 GB

**Recommendation:** Keep at least 5 backups, prune older ones regularly.

## Path Handling

### Windows Path Separators

- `codex-doctor` normalizes paths internally
- Accepts both `/` and `\` as input
- SQLite rollout paths stored with forward slashes for cross-platform compatibility

### Long Paths (Windows)

- Windows has a 260-character path limit by default
- Enable long path support: `Computer Configuration > Administrative Templates > System > Filesystem > Enable Win32 long paths`
- Or use `\\?\` prefix for paths exceeding 260 characters

## Concurrent Access

### Multiple codex-doctor Instances

**Not Supported:** Running multiple `codex-doctor` repair operations on the same `.codex` directory simultaneously is undefined behavior.

**Reason:**
- No distributed locking mechanism
- Backup creation is not atomic across instances
- SQLite writes may conflict

**Recommendation:** Use a wrapper script with file-based locking if automation is required.

### Codex Running During Repair

**Not Supported:** Running repairs while Codex is actively using the database.

**Detection:**
- `codex-doctor` attempts to open SQLite with a short timeout
- If locked, marks operation as `skipped` with `retryable: true`
- User must close Codex and retry

## Permission Requirements

### Unix-like Systems (macOS, Linux)

- Read/write access to `.codex` directory
- Read/write access to backup directory
- No root/sudo required (unless `.codex` has restrictive permissions)

### Windows

- Read/write access to `.codex` directory
- Read/write access to backup directory
- No administrator privileges required (unless `.codex` is in a protected location)

## Known Platform-Specific Issues

### Windows Defender / Antivirus

**Issue:** Real-time scanning may lock SQLite databases briefly during repair.

**Mitigation:**
- Add `.codex` directory to antivirus exclusions (if safe to do so)
- Retry repair if initial attempt reports locked database

### macOS Spotlight Indexing

**Issue:** Spotlight may briefly lock files during indexing.

**Mitigation:**
- Add `.codex` to Spotlight privacy exclusions if frequent lock conflicts occur
- Retry repair if initial attempt reports locked files

### Linux SELinux / AppArmor

**Issue:** Mandatory access control may prevent file operations.

**Mitigation:**
- Ensure `codex-doctor` runs in the same security context as Codex
- Check audit logs if permission denied errors occur

## Testing Recommendations

### Before Production Use

1. **Test on a copy:** Copy `.codex` to a test location and run repairs there first
2. **Verify backups:** Ensure backup creation and restore work on your platform
3. **Check locks:** Run `--dry-run` while Codex is running to verify lock detection
4. **Validate restore:** Restore a backup and verify Codex can read it

### Continuous Validation

- Run `codex-doctor scan` periodically to detect issues early
- Keep at least 3-5 backups for rollback safety
- Test restore procedure at least once to ensure familiarity

## Future Improvements

Potential enhancements for better platform compatibility:

- **Graceful degradation:** Partial repairs when some files are locked
- **Lock retry with backoff:** Automatic retry with exponential backoff
- **Distributed locking:** Support for concurrent repair prevention
- **Incremental backups:** Reduce backup size and time for large directories

## Summary

| Platform | SQLite Locking | File Locking | Backup Performance | Special Notes |
|----------|----------------|--------------|-------------------|---------------|
| Windows | Mandatory | Exclusive | Fast (local) | Close Codex first, watch antivirus |
| macOS | Advisory | Advisory | Fast (APFS CoW) | Watch Spotlight indexing |
| Linux | Advisory | Advisory | Fast (btrfs/XFS) | Check SELinux/AppArmor |

**Golden Rule:** Always close Codex before running repairs.
