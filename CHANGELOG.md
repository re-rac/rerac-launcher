# Changelog

Each `## vX.Y.Z` section below becomes the notes of that GitHub release. Keep one paragraph per line: GitHub shows
line breaks in release notes as they are.

## v0.2.1-alpha

A fix for macOS: v0.2.0-alpha could be refused as "damaged" after its first launch. The app is now signed as a whole, so macOS asks once whether to open it: go to System Settings → Privacy & Security and choose Open Anyway. The Version Management button on the game screen now opens the Official releases.

## v0.2.0-alpha

The first test release of the ReRAC launcher. It sets up ReRAC, the native rewrite of Ratchet & Clank (2002), from your own disc: point it at an image of your NTSC-U disc (SCUS-97199), and it checks the disc, copies the game data once and starts the game with one click. It downloads ReRAC itself from the official releases, so you can install a new version or switch back to an older one from the Versions settings. From the game screen you can also export the game's textures, sounds, models and levels, or extract the data again if something went wrong. Mod support comes later.

This is an early alpha, so expect rough edges. The macOS app is not signed yet, so macOS blocks it the first time you open it: go to System Settings → Privacy & Security and choose Open Anyway. The Windows installer isn't signed either and has never been tried; if Windows SmartScreen warns you, choose More info → Run anyway. The launcher contains no game files; you need your own copy of the game.
