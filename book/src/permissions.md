# File Permissions in Hoard

Hoard supports the three most popular desktop operating systems: Windows, macOS, and Linux.
One of these uses a very different implementation of file permissions compared to the others
-- this is why Rust only provides one bit of support for all platforms: whether something is
[readonly](https://doc.rust-lang.org/stable/std/fs/struct.Permissions.html) or not.

Previous versions of Hoard ignored this and just hoped things would stay consistent. With the
release of 0.5.0, though, Hoard added support for setting file permissions on restore.

## Configuration

As of 0.5.0, Hoard supports setting [configurable permissions](config/hoards-piles.md#file-permissions)
on files and folders on a `hoard restore`.

## When Permissions Are Set

Permissions are set on both backup and restore.

> Note: discussion of what permissions are set only apply to Unix-like systems, as Windows only
> supports `readonly`, which always defaults to `false` for the owning user.

### Backing Up

When backing up files, all files are given a mode of `0600` and all folders are given a mode of `0700`,
i.e., owner-only access. This is done to provide a little extra filesystem-based security, since the
permissions in the Hoard do not affect the permissions given on restore.

### Restoring

When restoring files, all files and folders are given the permissions specified in the most-specific
parent pile config. That is, the usual precedence holds, and permissions are not merged.

If no permissions are configured, the defaults are `0600` for files and `0700` for folders.