# Hoard

`hoard` is a program for backing up files from across a filesystem into a single directory
and restoring them later.

Most people will know these programs as "dotfile managers," where dotfiles are configuration
files on *nix (read: non-Windows) systems. Files on *nix systems are marked as hidden by
starting the file name with a dot (`.`).

## Terminology

- "Environment": An identifiable system configuration consisting of zero or more each of:
  operating system, hostname, environment variable, executables in `$PATH`, and/or existing
  paths,
- "Pile": A single file or directory with multiple possible paths where it can be found
  depending on the environment(s).
- "Hoard": One of:
  - A single anonymous pile.
  - One or more named, related piles.

## Usage

### Subcommands

- Backup: `hoard [flags...] backup [name] [name] [...]`
  - Back up the specified hoard(s). If no `name` is specified, all hoards are backed up.
- Restore: `hoard [flags...] restore [name] [name] [...]`
  - Restore the specified hoard(s). If no `name` is specified, all hoards are restored.
- Validate: `hoard [flags...] validate`
  - Attempt to parse the default configuration file (or the one provided via `--config-file`)
    Exits with code `0` if the config is valid.

### Flags

- `--help`: View the program's help message.
- `-V/--version`: Print the version of `hoard`.
- `-c/--config-file`: Path to (non-default) configuration file.
- `-h/--hoards-root`: Path to (non-default) hoards root directory.

### Verbosity

Output verbosity is controlled by the logging level. You can set the logging level with the
`HOARD_LOG` environment variable. Valid values (in decreasing verbosity) are:

- `trace`
- `debug`
- `info`
- `warn`
- `error`

The default logging level is `info` for release builds and `debug` for debugging builds.

### Default file locations

- Configuration file
  - Linux: `$XDG_CONFIG_HOME/hoard/config.toml` or `/home/$USER/.config/hoard/config.toml`
  - macos: `$HOME/Library/Application Support/com.shadow53.hoard/`
  - Windows: `C:\Users\$USER\AppData\Roaming\shadow53\hoard\config.toml`
- Hoards root
  - Linux: `$XDG_DATA_HOME/hoard/hoards` or `/home/$USER/.local/share/hoard/hoards`
  - macos: `$HOME/Library/Application Support/com.shadow53.hoard/hoards`
  - Windows: `C:\Users\$USER\AppData\Roaming\shadow53\hoard\data\hoards`

More specifically, `hoard` uses the [`directories`](https://docs.rs/directories) library,
placing the configuration file in the `config_dir` and the hoards root in the `data_dir`.

## Configuration

See [`config.toml.sample`](config.toml.sample) for a documented example configuration
file.

### Environments

Environments can be matched on one or more of five possible factors:

- `os`: [Operating System](https://doc.rust-lang.org/stable/std/env/consts/constant.OS.html)
- `env`: Environment variables
  - Can match on just existence or also a specific value.
- `hostname`: The system hostname.
- `exe_exists`: Whether an executable file exists in `$PATH`.
- `path_exists`: Whether something exists (one of) the given path(s).

All the above factors can be written using two-dimensional array syntax. That is,
`["foo", ["bar, "baz"]]` is interpreted as `(foo) OR (bar AND baz)`, in whatever way applies
to that given factor.

It is an error to include an `AND` condition for `os` or `hostname`, as a system can only have
one of each.

```toml
[envs]
[envs.example_env]
    # Matching something *nix-y
    os = ["linux", "freebsd"]
    # Either sed and sh, or bash, must exist
    exe_exists = ["bash", ["sh", "sed"]]
    # Require both $HOME to exist and $HOARD_EXAMPLE_ENV to equal YES.
    # Note the double square brackets that indicate AND instead of OR.
    env = [[
      { var = "HOME" },
      { var = "HOARD_EXAMPLE_ENV", expected = "YES" },
    ]]
```

### Exclusivity

The exclusivity lists indicate names of environments that are considered mutually exclusive to
each other -- that is, cannot appear in the same environment condition -- and the order indicates
which one(s) have precedence when matching environments.

See the [example config file](config.toml.sample) for a more thorough example.

```toml
exclusivity = [
    # Assuming all else the same, an environment condition string with "neovim" will take
    # precedence over one with "vim", which takes precedence over one with "emacs".
    ["neovim", "vim", "emacs"]
]
```

### Hoards

Hoards consist of one or more piles, where each pile is a mapping of *environment condition
strings* to paths on the filesystem.

An *environment condition string* is one or more environment names separated by pipes. The
system must match ALL environments in the string in order for the associated path to be
considered.

The following rules determine which path to use for a pile:

1. The condition string with the most environments wins.
2. If multiple conditions have the most environments, the exclusivity list is used to
   determine if one takes precedence.
3. If multiple conditions have the same precedence, an error is printed and `hoard` exits.
4. If no conditions match, the pile is skipped and a warning is printed.

```toml
[hoards]
# This hoard consists of a single anonymous pile
[hoards.simple_hoard]
    # This is "foo" and "bar" separated by a pipe character (`|`).
    # It will use this path if the system matches both environments "foo" and "bar".
    "foo|bar" = "/path/to/a/thing"
    # This path is considered if the system matches the environment "baz".
    # It will use this path if one of "foo" or "bar" doesn't match. Otherwise, "foo|bar"
    # takes precedence because it is a longer condition (more environments to match).
    "baz" = "/some/different/path"

[hoards.complex_hoard]
# This hoard consists of two named piles: "first" and "second".
[hoards.complex_hoard.first]
    "foo|bar" = "/some/path/first"
    "baz" = "/some/different/path/first"
[hoards.complex_hoard.second]
    "foo|bar" = "/some/path/second"
    "baz" = "/some/different/path/second"
```

### Pile Configuration

Pile configuration can be defined at three different levels:

1. Globally
2. Per-Hoard
3. Per-Pile

For a given Pile, any/all three of the levels of configuration are "layered" together, as appropriate for each
configuration item:

- Ignore patterns are merged and deduplicated.
- Encryption settings will use the most-specific settings.

#### Ignore Patterns

Set `ignore` to a list of [glob patterns](https://en.wikipedia.org/wiki/Glob_(programming)) indicating files and folders
to ignore. These lists will be merged across all levels of configuration.

```toml
# ... snip env definitions of "foo" and "bar" ...

# Top-level config, applies to all hoards
[config]
    # Ignore the .git folder at any depth
    ignore = ["**/.git"]

[hoards]
[hoards.anon_hoard]
    "foo" = "/some/path"
    "bar" = "/some/other/path"
[hoards.anon_hoard.config]
    ignore = [
        "**/.*", # Ignore all hidden files on Linux/macOS
        "*.log", # Ignore all top-level log files
    ]
[hoards.named_hoard]
[hoards.named_hoard.config]
    ignore = ["ignore-in-named-only"]
[hoards.named_hoard.pile1]
    "foo" = "/some/named/path"
    "bar" = "/another/named/path"
```
