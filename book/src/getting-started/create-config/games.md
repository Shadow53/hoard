# Example: *Mindustry* and *Death and Taxes*

This example explores the following concepts:

- Multiple named piles in a single hoard
- Mutually exclusive environments
- How to handle files for flatpak'd applications on Linux

For this example, we will consider two games, installed on both Windows and Linux. Further, we will consider multiple
methods of installing each game on each system. This is likely more work than one would do in practice, but this is for
sake of example.

# 1. Choose files to back up

Generally speaking, save files are usually found in one of the following locations:

- User documents
- The game's installation directory
- Some hidden directory (e.g. under `$XDG_CONFIG_HOME` or `$XDG_DATA_HOME` on Linux, `%APPDATA%` on Windows, etc.)

Of the two games we will be using, *Mindustry* sometimes stores its saves in the installation directory, while
*Death and Taxes* stores its saves in a game-specific subdirectory of a location common to all Unity games.

- *Mindustry*:
  - Flatpak: `${HOME}/.var/app/com.github.Anuken.Mindustry/data/Mindustry/saves/saves`
  - Linux Itch: `${XDG_DATA_HOME}/Mindustry/saves/saves`
  - Linux Steam: `${XDG_DATA_HOME}/Steam/steamapps/common/Mindustry/saves/saves`
  - Linux Steam Flatpak: `${HOME}/.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/common/Mindustry/saves/saves`
  - Windows Itch: `${APPDATA}/Mindustry/saves/saves`
  - Windows Steam: `${ProgramFiles(x86)}/Steam/steamapps/common/saves/saves`
- *Death and Taxes*:
  - Linux: `${XDG_CONFIG_HOME}/unity3d/Placeholder Gameworks/Death and Taxes/Saves`
  - Windows: `${USERPROFILE}/AppData/LocalLow/Placeholder Gameworks/Death and Taxes/Saves`

# 2. Add configuration for those files

We'll need environments for the operating systems and for the game stores. For
simplicity, we will assume that the XDG variables are always set. We'll also need to specify the order of precedence
for the game stores, since all three of them could be installed at once time.

> The double square brackets (`[[]]`) are used to indicate that *all* of the XDG environment variables are set. If
> single square brackets (`[]`) were used, it would mean that *at least one* must be set.
> 
> For more, see the documentation for [environments](../../config/environments.md).

```toml
exclusivity = [
    ["flatpak_steam", "flatpak_mindustry", "linux_steam", "linux_itch"],
    ["win_steam", "win_itch"],
]

[envs]
[envs.flatpak_mindustry]
    exe_exists = ["flatpak"]
    os = ["linux"]
    path_exists = [
      "/var/lib/flatpak/app/com.github.Anuken.Mindustry",
      "${XDG_DATA_HOME}/flatpak/app/com.github.Anuken.Mindustry",
    ]
[envs.flatpak_steam]
    exe_exists = ["flatpak"]
    os = ["linux"]
    path_exists = [
      "/var/lib/flatpak/app/com.valvesoftware.Steam",
      "${XDG_DATA_HOME}/flatpak/app/com.valvesoftware.Steam",
    ]
[envs.linux_itch]
    os = ["linux"]
    path_exists = ["${HOME}/.itch/itch"]
[envs.linux_steam]
    os = ["linux"]
    exe_exists = ["steam"]
[envs.win_itch]
    os = ["windows"]
    path_exists = ["${LOCALAPPDATA}/itch/itch-setup.exe"]
[envs.win_steam]
    os = ["windows"]
    path_exists = ["${ProgramFiles(x86)}/Steam/steam.exe"]
    
[hoards]
[hoards.game_saves]
[hoards.game_saves.death_and_taxes]
    "linux" = "${XDG_CONFIG_HOME}/unity3d/Placeholder Gameworks/Death and Taxes/Saves"
    "windows" = "${USERPROFILE}/AppData/LocalLow/Placeholder Gameworks/Death and Taxes/Saves"
[hoards.game_saves.mindustry]
    "flatpak_mindustry" = "${HOME}/.var/app/com.github.Anuken.Mindustry/data/Mindustry/saves/saves"
    "flatpak_steam" = "${HOME}/.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/common/Mindustry/saves/saves"
    "linux_itch" = "${XDG_DATA_HOME}/Mindustry/saves/saves"
    "linux_steam" = "${XDG_DATA_HOME}/Steam/steamapps/common/Mindustry/saves/saves"
    "win_itch" = "${APPDATA}/Mindustry/saves/saves"
    "win_steam" = "${ProgramFiles(x86)}/Steam/steamapps/common/saves/saves"
```

# 3. Do an initial backup

You can now run `hoard backup game_saves` to back up the game saves, and `hoard restore game_saves` to restore them.
