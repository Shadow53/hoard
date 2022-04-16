# Hoards and Piles

Hoards consist of one or more piles, where each pile is a mapping of *environment condition
strings* to paths on the filesystem.

An *environment condition string* is one or more environment names separated by pipes. The
system must match ALL environments in the string in order for the associated path to be
considered.

The following rules determine which path to use for a pile:

1. The condition string with the most environments wins.
2. If multiple conditions tie for most environments, the exclusivity list is used to
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

## Environment Variables

Paths may contain environment variables. Environment variables *must* be written as `${ENVVAR}`,
where `ENVVAR` is the environment variable. As an example, the following hoard could be used
to back up a user's `Documents` folder on Linux and Windows.

```toml
[hoards.documents]
    "linux" = "${HOME}/Documents"
    "windows" = "${USERPROFILE}/Documents"
```

For the user `myuser`, this expands to the following:

```toml
[hoards.documents]
    "linux" = "/home/myuser/Documents"
    "windows" = "C:/Users/myuser/Documents"
```

If the environment variable does not exist (i.e. is not defined), an error is returned and
the operation is canceled.

### Limitations

1. There is no support for default values, i.e. `${MYVAR:-"/some/default"}`

## Pile Configuration

Pile configuration can be defined at three different levels:

1. Globally
2. Per-Hoard
3. Per-Pile

For a given Pile, any/all three of the levels of configuration are "layered" together, as appropriate for each
configuration item:

- Hashing algorithms use the most-specific layer, or the default if not specified.
- Ignore patterns are merged and deduplicated.
- Encryption settings will use the most-specific settings.

### Hashing Algorithms

Set `hash_algorithm` to one of the below strings to manually set which hashing algorithm is used when recording 
[Hoard operations](../cli/checks.md#remote-operations).

- `"sha256"` (default): SHA-256 is an older but unbroken algorithm.
- `"md5"`: MD5 is a quick algorithm but also cryptographically broken. Supported for compatibility with 
  an older operation log format and should be avoided.

### Ignore Patterns

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

### File Permissions

> For a general discussion of file/folder permission support in Hoard, including
> Windows-specific limitations, see [this page](../../permissions.md).

Hoard supports setting permissions separately for files and folders, using `file_permissions`
and `folder_permissions`, respectively. These can be specified in two different ways: a "mode"
and boolean flags.

#### Mode

A "mode" is an octal (base 8) integer representing read, write, and execute permissions for the
owning user, users in the owning "group", and all other users. See the
[Wikipedia article](https://en.wikipedia.org/wiki/Unix_file_types#Representations) for more.

You can specify a "mode" in TOML by prefixing the number with `0o`. For example, a common file
mode is `0o644` (read/write for the owner user, readonly for everyone else).

```toml
[config]
    file_permissions = 0o644
```

If using a "mode" is too confusing, you can also use a set of boolean flags: just set these to
`true` or `false`:

#### Flags

> Note: only `is_writable` is supported on Windows. All other flags are ignored.
> Note 2: "others" in the context of these boolean flags are a combination of the "group" and "other"
> values from a file "mode".

- `is_readable`: the owning user can read the contents of the file or folder. This should not be
  set to `false` and is provided for completeness' sake.
- `is_writable`: the owning user can modify and delete the file or folder.
- `is_executable`: this has different meanings depending on whether it applies to files or folders:
  - `true` for files means that the user can run the file as an executable program.
  - `true` for folders means that the user can list the contents of the folder.
  - In short, this should always be `true` for folders.

- `others_can_read`: like `is_readable` but for non-owner users.
- `others_can_write`: like `is_writable` but for non-owner users.
- `others_can_execute`: like `is_executable` but for non-owner users.

```toml
# ... snip env definitions of "foo" and "bar" ...

# Top-level config, applies to all hoards
[config]
    # These represent the current defaults used by Hoard:
    # owner-only access.
    file_permissions = 0o600
    folder_permissions = 0o700

[hoards]
[hoards.anon_hoard]
    "foo" = "/some/path"
    "bar" = "/some/other/path"
[hoards.anon_hoard.config.file_permissions]
    # Equivalent to a 0o644 mode
    is_readable = true
    is_writable = true
    others_can_read = true
[hoards.anon_hoard.config.folder_permissions]
    # Equivalent to a 0o755 mode
    is_readable = true
    is_writable = true
    is_executable = true
    others_can_read = true
    others_can_execute = true
```