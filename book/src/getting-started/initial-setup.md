# Initial Setup

Hoard v0.5.2 added the `init` subcommand to create the folders and files necessary for Hoard to run.

> This command only needs to be run the first time you set Hoard up. After that, including on new
> machines, it is enough to synchronize the [hoard data directory][hoard-data-dir] to the new
> machine and setup or restore the [configuration file][hoard-config-file].

## Initializing Hoard

Run `hoard init`. Everything necessary will be created, including a sample configuration file.

Then, run `hoard edit` to [edit the new configuration file][edit-config-file].

[hoard-data-dir]: ../file-locations.md#hoard-data-directroy
[hoard-config-file]: ../file-locations.md#config-file
[edit-config-file]: ./create-config/
