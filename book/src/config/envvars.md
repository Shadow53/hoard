# Environment Variables

## Default Values

You may define default values for environment variables, should they not be set when `hoard` is run. If the variable
*is* set, the default value is ignored. Default values may include [interpolated](#interpolation) values of other
environment variables, including other variables with assigned defaults.

> Make sure there are no cyclical value definitions, as these will cause errors.
> 
> ```toml
> [defaults]
>     # This will never resolve
>     "SELF_CYCLICAL" = "I am ${SELF_CYCLICAL}"
>     # These will cause errors if both are undefined, but
>     # the errors will not be apparent if one is defined.
>     "MUTUALLY_CYCLICAL_1" = "I'm the sibling of ${MUTUTALLY_CYCLICAL_2}"
>     "MUTUALLY_CYCLICAL_2" = "I'm the sibling of ${MUTUALLY_CYCLICAL_1}"
> ```

### Examples

This example sets `$XDG_CONFIG_HOME` and `$XDG_DATA_HOME`, two variables that are commonly used on Unix-y systems to
determine where application configuration and data files should be kept.

```toml
[defaults]
    XDG_CONFIG_HOME = "${HOME}/.config"
    XDG_DATA_HOME = "${HOME}/.local/share"
```

## Interpolation

Environment variables may be interpolated into certain parts of the configuration file. Namely,

- [Hoard/Pile paths](./hoards-piles.md#environment-variables)
- [Environment variable default values](#default-values)

Interpolate a variable using `${VAR}`, where `VAR` is the name of the variable. See the above links for specific
examples.