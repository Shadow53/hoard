# Example: Vim and Neovim

This example explores the following concepts:

- Multiple named piles in a single hoard
- Mutually exclusive environments
- Ignoring files by glob pattern

For simplicity, this example assumes only one operating system (Linux) will ever be used. For an example that defines
paths based on operating system, see the [Hoard Config](./hoard.md) example.

## 1. Choose files to back up

While a Vim configuration can live inside a single file, we will consider a situation where there is a directory
called `config` whose contents are included into the main file with the following code:

```vimscript
if has('nvim')
    runtime! config/*.vim
else
    runtime! ~/.vim/config
endif
```

In this situation, the configuration files are found in the following locations:

- Vim:
    - Config entrypoint: `${HOME}/.vimrc`
    - `config` directory: `${HOME}/.vim/config`
- Neovim:
    - Config entrypoint: `${XDG_CONFIG_HOME}/nvim/init.vim`
    - `config` directory: `${XDG_CONFIG_HOME}/nvim/config`

## 1.1. Choose files to ignore

For sake of example, let's suppose the `config` directory contains a number of old, unused files whose names
end with `.backup`. You're going to get around to deleting them eventually, but they might have code you want
to keep, just not backed up.

## 2. Add configuration for those files

As stated above, for simplicity we are assuming that Linux is the only operating system being used -- if it were
not, we would need to figure out the paths for other operating systems and include configuration conditional on
that. Since we are not worried about that, though, the only environments we care about are whether Vim and/or
Neovim are installed:

```toml
[envs]
    # Checks for CLI Vim *or* GUI (Gtk+) Vim
    vim = { exe_exists = ["vim", "gvim"] }
    # Checks for CLI Neovim *or* GUI (Qt) Neovim
    neovim = { exe_exists = ["nvim", "nvim-qt"] }
```

Since it is possible for both Vim and Neovim to be installed on the same system, we need to tell Hoard which
one to prioritize. In this case, we will prioritize Neovim:

```toml
exclusivity = [
    ["neovim", "vim"]
]

[envs]
    # Checks for CLI Vim *or* GUI (Gtk+) Vim
    vim = { exe_exists = ["vim", "gvim"] }
    # Checks for CLI Neovim *or* GUI (Qt) Neovim
    neovim = { exe_exists = ["nvim", "nvim-qt"] }
```

Finally, define the actual hoard. We'll call it `vim`:

```toml
[hoards]
[hoards.vim]
[hoards.vim.config]
    # This is the configuration for the vim hoard and is include for
    # demonstration only. For this example, you should use the config
    # *inside* the config_dir pile instead.
    ignore = ["**/*.backup"]
[hoards.vim.init]
    "vim" = "${HOME}/.vimrc"
    "neovim" = "${XDG_CONFIG_DIR}/nvim/init.vim"
[hoards.vim.config_dir]
    # This is configuration just for the vim.config_dir pile
    config = { ignore = ["**/*.backup"] }
    "vim" = "${HOME}/.vim/config"
    "neovim" = "${XDG_CONFIG_DIR}/nvim/config"
```

**NOTE:** The name `config` is reserved for [hoard/pile configuration](../../config/hoards-piles.md#pile-configuration)
and cannot be used as the name of a hoard or pile. This is why the name `config_dir` is used above: using `config` would
conflict with the hoard-level configuration block.

We use the glob pattern `**/*.backup` above to indicate that any file in any subdirectory of `config/` with suffix
`.backup` should be ignored. Use `*.backup` for top-level files only.

## 3. Do an initial backup

You can now run `hoard backup vim` to back up your Vim/Neovim configuration, and `hoard restore vim` to restore the
latest backup.
