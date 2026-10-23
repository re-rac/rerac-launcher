# Changelog

Each `## vX.Y.Z` section below becomes the notes of that GitHub release. Keep one paragraph per line: GitHub shows
line breaks in release notes as they are.

## v0.3.1-alpha

Installing Going Commando no longer fails just before it finishes.

## v0.3.0-alpha

Each game now has its own version, chosen from the Version list right above the Play button. When a newer release is out, the list says so and offers it, and nothing downloads until you pick it. Version Management in the settings has a tab for each game, and a downloaded version can be deleted there.

Going Commando has its own page now. Its releases will show up there once they are published.

From this version on, the launcher keeps itself up to date: it downloads a new launcher in the background and asks you to restart when it is ready, never while a game is running. This one version still has to be installed by hand.

## v0.2.1-alpha

A fix for macOS: v0.2.0-alpha could be refused as "damaged" after its first launch. The app is now signed as a whole, so macOS asks once whether to open it: go to System Settings → Privacy & Security and choose Open Anyway. The Version Management button on the game screen now opens the Official releases.

## v0.2.0-alpha

The first test release of the ReRAC launcher. It sets up ReRAC, the native rewrite of Ratchet & Clank (2002), from your own disc: point it at an image of your NTSC-U disc (SCUS-97199), and it checks the disc, copies the game data once and starts the game with one click. It downloads ReRAC itself from the official releases, so you can install a new version or switch back to an older one from the Versions settings. From the game screen you can also export the game's textures, sounds, models and levels, or extract the data again if something went wrong. Mod support comes later.

This is an early alpha, so expect rough edges. The macOS app is not signed yet, so macOS blocks it the first time you open it: go to System Settings → Privacy & Security and choose Open Anyway. The Windows installer isn't signed either and has never been tried; if Windows SmartScreen warns you, choose More info → Run anyway. The launcher contains no game files; you need your own copy of the game.
