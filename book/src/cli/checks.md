# Pre-Operation Checks

To help protect against accidentally overwriting or deleting files, `hoard` runs some consistency
checks prior to running any operations.

To skip running the checks, run `hoard` with the `--force` flag. There is not currently a way to disable
individual checks.

## Last Paths

This check compares the paths used previously with a given hoard to the ones resolved for the current
operation. If any of these paths differ, a warning is displayed and the operation(s) canceled.

## Remote Operations

By default, `hoard` logs information about successful operations to a directory that is intended to be
synchronized with the main hoards directory. This information is used to determine if a given file was
last modified by a remote system. If so, a warning is displayed and the operation(s) canceled.
