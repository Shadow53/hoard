# Hoard

[![Netlify Status](https://api.netlify.com/api/v1/badges/b91e71ce-673e-466c-a6ff-2b877ec0dd97/deploy-status)](https://app.netlify.com/sites/hoard-docs/deploys)
[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2FShadow53%2Fhoard.svg?type=shield)](https://app.fossa.com/projects/git%2Bgithub.com%2FShadow53%2Fhoard?ref=badge_shield)

`hoard` is a program for backing up files from across a filesystem into a single directory
and restoring them later.

Most people will know these programs as "dotfile managers," where dotfiles are configuration
files on *nix (read: non-Windows) systems. Files on *nix systems are marked as hidden by
starting the file name with a dot (`.`).

## Documentation

You can find all documentation at https://hoard.rs.

## Configuration

See [`config.toml.sample`](config.toml.sample) for a documented example configuration file.

## Testing

Hoard's runtime behavior depends on environment variables, which the tests override to prevent polluting the developer's
system and/or home directory. Because of this, tests must be run in one of two ways:

1. Single-threaded, using `cargo make test-single-thread` or `cargo test -- --test-threads=1`.
2. As separate processes with their own environments, using `cargo make test-nextest` or `cargo nextest run`.
  - `cargo-make` should install the dependency automatically. Otherwise, run `cargo install cargo-nextest`.

Tests can also be run in a container using `cargo make docker-tests`.

## License
[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2FShadow53%2Fhoard.svg?type=large)](https://app.fossa.com/projects/git%2Bgithub.com%2FShadow53%2Fhoard?ref=badge_large)
