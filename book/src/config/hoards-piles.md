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

## Pile Configuration

Pile configuration can be defined at three different levels:

1. Globally
2. Per-Hoard
3. Per-Pile

For a given Pile, any/all three of the levels of configuration are "layered" together, as appropriate for each
configuration item:

- Ignore patterns are merged and deduplicated.
- Encryption settings will use the most-specific settings.

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

