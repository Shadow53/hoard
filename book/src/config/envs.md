# Environments

The path used for a given pile depends on the best matching environment(s) for a configured path. This page
discusses how to define environments. For how to use them with hoards/piles, see
[Hoards and Piles](config/hoards-piles.md).

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

## Exclusivity

The exclusivity lists indicate names of environments that are considered mutually exclusive to
each other -- that is, cannot appear in the same environment condition -- and the order indicates
which one(s) have precedence when matching environments.

See the [example config file][example config] for a more thorough example.

[example config]: https://github.com/Shadow53/hoard/tree/main/config.toml.sample

```toml
exclusivity = [
    # Assuming all else the same, an environment condition string with "neovim" will take
    # precedence over one with "vim", which takes precedence over one with "emacs".
    ["neovim", "vim", "emacs"]
]
```
