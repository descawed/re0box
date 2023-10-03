# re0box
An item box mod for Resident Evil 0 HD on PC.

## Usage
Download the latest release and unzip it to your Resident Evil 0 folder (if you don't know where it is, right-click on
the game in Steam and choose Manage > Browse local files). It should then load automatically when you run the game.
There are no physical item boxes; instead, you can access the box at any typewriter, even if you don't have an ink
ribbon. When you select the option to open the box, the inventory menu will open, and you'll see the contents of the box
on the left side of the screen where your partner's inventory is normally displayed. There's no visible scrollbar, but
you can scroll by moving the cursor up from the top of the inventory or down from the bottom. Unlike some games in the
series, the box inventory is not circular, so you can't scroll past the top to get to the bottom or vice versa. By
default, the Leave option in the inventory is disabled while the mod is active, but you can change that in the config
file (see the Configuration section below).

### Important Notes
- Only the legal Steam version is supported.
- If you uninstall the mod and save, all your item boxes in all your saves will be deleted. If you want to play without
  the mod but still keep your item boxes in your other saves, you should open the config file (see Configuration section
  below) and disable the mod by setting Mod=0. That will disable all the mod's features but prevent the game from
  deleting your item boxes in other saves.
- This mod supports all the languages that I can select in my version of the game, which are Simplified Chinese,
  Traditional Chinese, English, French, German, Italian, Japanese, and Spanish. However, only English and German have
  updated typewriter text that mentions the item box. If you're playing in one of the other languages I mentioned,
  you'll still get the prompt to the use the box, but the text won't say anything about it, so it may be a little
  confusing. If someone wants to send me appropriate text for the other languages, I'll update the mod. If you're
  playing in a language that's NOT one of the ones I mentioned, you won't get the prompt at all and the mod will be
  unusable for you. To add support for other languages, I'd need someone to go into the game folder and send me a copy
  of the "nativePC\arc\message\msg_(lang).arc" file for that language, along with updated text for the typewriter
  prompts. Even then, if the .exe is different in your region, the mod still won't work (but I don't know if it would
  be).

### Configuration
The mod comes with a configuration file called re0box.ini with a couple options. You'll find it in your Resident Evil 0
folder and you can open it with any text editor. Note that changes to this file won't take effect while the game is
running; you'll need to restart the game for it to pick up any changes.

**Enable**

This section controls whether mod features are enabled.

- Mod: this controls whether the mod is enabled. When Mod=1 (which is the default), the mod is enabled. If you change it
  to Mod=0, the mod is disabled.
- Leave: this controls whether you're allowed to drop items (the "Leave" option in the inventory). The default is
  Leave=0, meaning you're not allowed to drop items, because I think that having both the item box and the ability to
  drop items is OP. But if you want both, you can change it to Leave=1, and then you'll be able to drop items and still
  access the item box.

**Log**

This section controls logging behavior.

- Level: this controls how much information is logged. The options are off, error, warn, info, debug, and trace, where
  each option logs progressively more information. High log levels (such as trace) may impact performance but are
  useful for troubleshooting issues like crashes. The default is info.
- Path: path to the log file, relative to the game folder. The default is re0box.log. If you don't want to go in the
  game folder, you can also use an absolute path for a different location, such as C:\Users\Bob\Documents\re0box.log.

## Uninstall
Delete scripts\re0box.asi from the Resident Evil 0 folder. None of the other mod files will have any effect once that's
gone, but if you want to purge everything, this is the full list of files added by the mod:
- dinput8.dll (note that other mods may need this file. if you have anything in your scripts folder besides re0box.asi,
  you should leave this one alone.)
- re0box.ini
- re0box.log
- re0box_readme.txt
- nativePC\arc\message\msg_chS_box.arc
- nativePC\arc\message\msg_chT_box.arc
- nativePC\arc\message\msg_eng_box.arc
- nativePC\arc\message\msg_fre_box.arc
- nativePC\arc\message\msg_ger_box.arc
- nativePC\arc\message\msg_ita_box.arc
- nativePC\arc\message\msg_jpn_box.arc
- nativePC\arc\message\msg_spa_box.arc
- scripts\re0box.asi

## Build
This mod is written in Rust. The default target is i686-windows-pc-gnu because RE0 is a 32-bit game and I'm
cross-compiling from Linux. I imagine the MSVC toolchain would also work but I haven't tested it. As long as Rust and
the appropriate toolchain are installed, you should just be able to do a `cargo build`. The mod is currently distributed
as an ASI plugin using [Ultimate-ASI-Loader](https://github.com/ThirteenAG/Ultimate-ASI-Loader) as the loader. This
helps ensure compatibility with other DLL-based mods. Just rename re0box.dll to re0box.asi and put it in the scripts
folder.

Aside from the DLL itself, we also have to edit the game's message files so typewriters prompt to use the box. These are
found in nativePC\arc\message. There's one file for each language the game supports, named in the format
`msg_(lang).arc`, where (lang) is a three-character language ID. Rather than replace the original files, which would
cause problems if the user wants to uninstall the mod or if they verify their game files with Steam, the mod looks for
message files named `msg_(lang)_box.arc`. You can use FluffyQuack's
[ARCtool](https://residentevilmodding.boards.net/thread/481/) to extract these files. Within the extracted folder,
the message file containing the typewriter messages is found at `message\message_commonmsg_(lang).gmd`. The repo's data
folder contains xdelta patches for this file for various languages that you can apply to add the updated typewriter
message. Then you can use ARCtool to repack the file. If you want to edit the GMD files yourself, it's easy enough to do
in a hex editor as long as you're only editing the message text. The text for all the messages are the last thing in the
file delimited by null bytes. There's a little-endian u32 at offset 0x20 in the file that gives the size of the message
block in bytes. Make whatever edits to the messages you want, then add the net number of bytes you added/removed to that
value. Repack the arc file and your edits should show up in game.

## Credits
This mod was made by descawed. I used a number of existing tools in the making of this mod; special thanks to:
- hasherezade for [dll_injector](https://github.com/hasherezade/dll_injector)
- ThirteenAG for [Ultimate ASI Loader](https://github.com/ThirteenAG/Ultimate-ASI-Loader)
- FluffyQuack for [ARCtool](https://residentevilmodding.boards.net/thread/481/)
- onepiecefreak3 for [GMDConverter](https://github.com/onepiecefreak3/GMDConverter)
- ErnestJugend for the Spanish translation