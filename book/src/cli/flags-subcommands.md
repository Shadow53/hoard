# Flags

Flags can be used with any subcommand and must be specified *before* any subcommand.

- `--help`: View the program's help message.
- `-V/--version`: Print the installed version of `hoard`.
- `-c/--config-file`: Path to (non-default) configuration file.
- `-h/--hoards-root`: Path to (non-default) hoards root directory.

# Subcommands

- Backup: `hoard [flags...] backup [name] [name] [...]`
  - Back up the specified hoard(s). If no `name` is specified, all hoards are backed up.
- Restore: `hoard [flags...] restore [name] [name] [...]`
  - Restore the specified hoard(s). If no `name` is specified, all hoards are restored.
- Validate: `hoard [flags...] validate`
  - Attempt to parse the default configuration file (or the one provided via `--config-file`)
    Exits with code `0` if the config is valid.
