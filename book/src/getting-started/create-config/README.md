# Creating the Configuration File

## 0. Determine file Location

Check the [File Locations](../../file-locations.md#config-directory) page for the location that the configuration file should be placed.
If you are creating the file using your systems File Explorer, you may need to enable hidden files/folders. 

> In the future, there will be a `hoard edit` command to automatically create and open the file.

## 1. Choose files to back up

The next step is determining what you are going to back up with Hoard. Common examples are configuration files for
various programs and save files for PC games. Just like with Hoard's configuration file, these files are often found
in hidden folders, so you may have to do some digging to find them.

For the sake of this guide, we will consider three different programs:

> **NOTE:** The examples use TOML as the config file format. Users looking to use YAML should be able to translate 
> the configuration from TOML. See also [this other note](../../config/).

1. [Hoard itself](./hoard.md)
2. [Vim and Neovim](./vim.md)
3. [*Mindustry* and *Death and Taxes*](./games.md)

## 2. Add configuration for those files

When adding configuration for a specific file or set of files, consider:

- What to name the hoard and, optionally, the pile or piles within it. See the examples linked above for ideas of how
  to structure hoards.
- What conditions must be true for a path to be used. These determine the environments, or
  [`envs`](../../config/envs.md) that you will define.
- If there are multiple, mutually exclusive conditions that can be true at the same time (see 
  [Vim and Neovim](vim-neovim.md) for an example). This determines if you need to add anything under
  [`exclusivity`](../../config/envs.md#exclusivity).
- Whether the programs use environment variables to determine where to place files, or if it is hardcoded. This will
  inform whether you use environment variables in the pile path or not.
- Whether there are files in a directory that you want to [ignore](../../config/hoards-piles.md#ignore-patterns) when
  backing up.

## 2.1: Validate the configuration

When you think you have completed the configuration, double check by running `hoard validate`. If there are any errors
with the configuration file, this command will tell you.

## 3. Do an initial backup

Once you have validated the configuration, run `hoard backup <hoard name>`, where `<hoard name>` is the name of the
hoard you just created. Alternatively, you can run `hoard backup` to back up all configured hoards.

## 4. Optional: Set up sync

If you want to use Hoard to synchronize files between systems, you'll want to set up some sort of synchronization.
Hoard aims to be agnostic to which method is used and only requires that the data files can be found in the
[expected location](../../file-locations.md#hoard-data-directory). This can be done by synchronizing that directory
directly or by creating a symbolic link to another directory.

Possible sync solutions:

- [Syncthing](https://syncthing.net)
- A git repository on any hosting service
- File synchronization services like Nextcloud/ownCloud, Dropbox, Microsoft Onedrive, etc.

Whatever solution you choose, be aware of the possibility of synchronization conflicts. Hoard has no special logic to
prevent synchronization-level conflicts, instead leaving that to the synchronization software itself.
