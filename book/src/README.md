# Welcome to Hoard!

`hoard` is a program for backing up files from across a filesystem into a single directory
and restoring them later.

Most people will know these programs as "dotfile managers," where dotfiles are configuration
files on *nix (read: non-Windows) systems. Files on *nix systems are marked as hidden by
starting the file name with a dot (`.`).

`hoard` aims to be a little more useful than other dotfile managers:

1. Many dotfile managers store files in a structure based on their path relative to the user's home directory. This is
   useful in most cases, but can cause problems when wanted to share files across systems that don't use the same paths,
   e.g., Windows and Linux. `hoard` instead namespaces files based on the ["Hoard" and "Pile"](./terminology.md) they
   are configured in, then relative to the root of the Pile. This makes it easy to backup and restore files to very
   different locations.
   
2. Most dotfile managers do not prevent you from accidentally destructive behavior. See [Checks](cli/checks.md) for more
   information.
