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

## `hoard validate`

```
hoard [flags...] validate
```
Attempt to parse the default configuration file (or the one provided via `--config-file`).
Exits with code `0` if the config is valid.
