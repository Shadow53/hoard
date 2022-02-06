# Flags

Flags can be used with any subcommand and must be specified *before* any subcommand.

- `--help`: View the program's help message.
- `-V/--version`: Print the installed version of `hoard`.
- `-c/--config-file`: Path to (non-default) configuration file.
- `-h/--hoards-root`: Path to (non-default) hoards root directory.

# Subcommands

## `hoard backup`

```
hoard [flags...] backup [name] [name] [...]
``` 

Back up the specified hoard(s). If no `name` is specified, all hoards are backed up.

## `hoard cleanup`

```
hoard [flags...] cleanup
```

Deletes all extra [operation log files](../file-locations.md#history-files)
that are unnecessary for the related [check](./checks.md#remote-operations).

## `hoard diff`

```
hoard [flags...] diff [-v|--verbose] <name>
```

Shows a list of all files that differ between the system and the hoard given by `<name>`. This
can detect files that were created, modified, or deleted, locally or remotely.

If `-v` or `--verbose` is passed, the output will show unified diffs of text files.

## `hoard edit`

```
hoard [flags...] edit
```

Opens the Hoard configuration file in the default editor. This uses `$EDITOR` when set and
the system default handler otherwise.

- On Linux and BSD, this delegates to `xdg-open`, which must be installed if `$EDITOR` is not set.

## `hoard list`

```
hoard [flags...] list
```

List all configured hoards by name (sorted).

## `hoard restore`

```
hoard [flags...] restore [name] [name] [...]
```

Restore the specified hoard(s). If no `name` is specified, all hoards are restored.

## `hoard status`

```
hoard [flags...] status
```

Displays the current status of every configured hoard:

- `modified locally`: all changes are local, and this hoard can be safely backed up with
  `hoard backup`.
- `modified remotely`: all changes are remote, and this hoard can be safely applied locally
  with `hoard restore`.
- `mixed changes`: changes are a combination of local and remote, and manual intervention is
  recommended. Using [`hoard diff`](#hoard-diff) may be useful in reconciling changes.
- `unexpected changes`: at least one hoard file appears to have been directly modified instead
  of using `hoard backup`. [`hoard diff`](#hoard-diff) may be useful in handling the unexpected
  change.

## `hoard upgrade`

```
hoard [flags...] upgrade
```

Automatically upgrades hoard-related files to newer formats. Old formats may be removed in later
versions to help keep the codebase clean.

This currently affects:

- [Operation log files](checks.md#remote-operations)

## `hoard validate`

```
hoard [flags...] validate
```
Attempt to parse the default configuration file (or the one provided via `--config-file`).
Exits with code `0` if the config is valid.
