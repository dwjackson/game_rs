# games: command-line game manager

The `games` executable is used to run games on Linux from the command line.

I like running things from the command line, so this program is used to launch
games from the command line. It's kind of like if Steam or Lutris were far more
annoying to use. The games are manually configured via a TOML file in the
user's home directory. Native, Wine, DOSBox, and ScummVM games can be launched
via the `game` executable.


## Commands

* `list` - list all known games
* `play [GAME_ID]` - play the game with the given ID

## Configuration

The configuration file must be in the user's home directory and must be called
`games.toml`. 

### directories

The `[directories]` table contains directories that can be used to simplify
the creation of many games that all share a common parent directory.

Example:

```toml
[directories]
games_dir = "/home/test/Games"
wine_gog_dir = "/home/test/.wine/drive_c/GOG Games"
```

### games

The `[games]` table holds the actual game configurations. Each game is named
like `[games.GAME_ID]` where `GAME_ID` the the ID you want to use for the game.
Known fields are as follows:

* `cmd` - command to execute to run the game
* `dir` - directory from which to run the game command
* `dosbox_config` - the name of a DOSBox configuration file to use
* `env` - a table where each key/value pair corresponds to an environment
variable that should be set before running the game
* `prefix_dir` - the key of the entry in the `[directories]` table that is the
parent directory of the `dir`
* `scummvm_id` - The ScummVM target ID of the game to launch
* `wine_exe` - the name of the Windows executable for `wine` to execute

_Technically_ all of these fields are optional, but at least one of `cmd`,
`wine_exe`, `dosbox_config`, or `scummvm_id` is required.

## Dependencies

This program assumes that you have the following programs installed:

* Wine
* Mangohud
* DOSBox
* ScummVM

