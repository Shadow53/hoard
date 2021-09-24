# Environment

Not to be confused with an [environment variable][envvar]. An Environment is an identifiable system configuration
consisting of zero or more each of the following:

  - Operating system
  - Hostname
  - Environment variable
  - Executables in `$PATH`
  - Existing paths (folders/files) on the system

Multiple Environments can be mixed and matched in **Environment Strings** when defining what paths
to use for a given [Pile](#pile). Some Environments may be *mutually exclusive* with certain others.

# Pile

A single file or directory with multiple possible places where it can be found, depending on the
system configuration. The path to use is determined by the best matching Environment String.

# Hoard

A collection of one or more [Piles](#pile) that form a logical unit.

# Examples

Consider this configuration snippet (see [Configuration File](./config/index.md) for more explanation):

```toml
exclusivity = [
    ["neovim", "vim"],
]

[envs]
[envs.neovim]
    exe_exists = ["nvim", "nvim-qt"]
[envs.unix]
    os = ["linux", "freebsd"]
    env = [
        { var = "HOME" },
        { var = "XDG_CONFIG_HOME" }
    ]
[envs.vim]
    # Detect "vim" if AT LEAST one of `vim` or `gvim` exists in $PATH.
    exe_exists = ["vim", "gvim"]
[envs.windows]
    os = ["windows"]

[hoards]
[hoards.vim]
    [hoards.vim.init]
        "unix|neovim" = "${XDG_CONFIG_HOME}/nvim/init.vim"
        "unix|vim" = "${HOME}/.vimrc"
        "windows|neovim" = "${LOCALAPPDATA}\\nvim\\init.vim"
        "windows|vim" = "${USERPROFILE}\\.vim\\_vimrc"
    [hoards.vim.configdir]
        "windows|neovim" = "${LOCALAPPDATA}\\nvim\\config"
        "windows|vim" = "${USERPROFILE}\\.vim\\config"
        "unix|neovim" = "${XDG_CONFIG_HOME}/nvim/config"
        "unix|vim" = "${HOME}/.vim/config"
```

- Environments: `neovim`, `unix`, `vim`, `windows`; `neovim` and `vim` are mutually exclusive.
- Hoards: just one, called `vim`, containing two named Piles.
- Piles: `init` and `configdir`; `init` is the entry config file for a Vim program, while `configdir` is a directory
  containing more config files loaded by `init`.

Take a closer look at the `init` Pile. There are four possible paths the file can be at, based on a combination of which
operating system it is running on and whether Neovim or Vim is installed. The `exclusivity` line tells Hoard to prefer
Neovim if both are present.

[envvar]: https://en.wikipedia.org/wiki/Environment_variable
