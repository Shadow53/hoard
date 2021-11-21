# Example: Hoard itself

Let's start with Hoard itself as an example. This allows you to easily share your Hoard configuration across multiple
systems with only a little initial setup. Being a single file, it also makes for a simple example.

Depending on the operating system used, the configuration file can be in one of a
[number of locations](../../file-locations.md#config-directory). You will want to make entries for each system you
plan to use Hoard on. For this guide, we will use Windows and Linux as examples.

## 1. Choose files to back up

- Windows: `%APPDATA%\shadow53\hoard\config.toml`
- Linux: `$XDG_CONFIG_HOME/hoard/` or `$HOME/.config/hoard/`

The author uses the `XDG_CONFIG_HOME` path on Linux, but this variable is not always set by default, so this guide will
add some logic to cover both cases.

## 2. Add configuration for those files

Since this example expects Hoard to be used on multiple operating systems, we will create environments for each OS. We
will also add an extra environment for when the environment variable `XDG_CONFIG_HOME` is set.

```toml
[envs]
linux = { os = ["linux"] }
windows = { os = ["windows"] }
xdg_config_set = { env = [{ var = "XDG_CONFIG_HOME" }] }
```

The above configuration uses a shorthand syntax. The following is also valid TOML:

```toml
[envs]
[envs.linux]
    os = ["linux"]
[envs.windows]
    os = ["windows"]
[envs.xdg_config_set]
    env = [
        { var = "XDG_CONFIG_HOME" } 
    ]
```

Now that the environments are defined, we can create the hoard that will contain the configuration file.

```toml
[hoards]
[hoards.hoard_config]
    "windows" = "${APPDATA}/shadow53/hoard/config.toml"
    "linux" = "${HOME}/.config/hoard/config.toml"
    "linux|xdg_config_set" = "${XDG_CONFIG_HOME}/hoard/config.toml"
```

You will notice that the keys `"windows"`, `"linux"`, and `"linux|xdg_config_set"` are wrapped in double quotes. This is
because of the pipe character `|`, which is not allowed by default in TOML identifiers. The pipe indicates that multiple
environments must match -- in this case, `linux` and `xdg_config_set` must both match. The quotes around `"linux"` and
`"windows"` are merely for consistency.

# 3. Do an initial backup

You can now run `hoard backup hoard_config` to back up the configuration file, and `hoard restore hoard_config` to
restore the version from the hoard.
