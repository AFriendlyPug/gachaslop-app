# Changelog

What changed in each version of GachaSlop. When a version is tagged, its section becomes the GitHub release notes, which the app's update banner shows, so write it for the people using the app.

## [Unreleased]

## [0.3.1] - 2026-11-18

- Uses far less memory: the popup is freed 30 seconds after you close it and rebuilt when you open it (about 10 MB instead of about 200 MB while closed). Switch off "Free memory when closed" in Settings if you'd rather it opened instantly.
- An optional keyboard shortcut to open and close the popup from anywhere. It's off until you turn it on in Settings; it's Ctrl+Alt+G unless you pick another.
- Turn resets, heads-ups or banners off for one game at a time.
- Your own games can follow a server's time zone, daylight saving included, instead of a fixed UTC time.
- Game icons can be any image file, or a link the app downloads for you.
- The Notifications list can be filtered by game.
- The update banner shows what's new.
- "Show as a tray icon" is now a setting that's remembered.
- The window fits the cards on the home screen, growing and shrinking with them. Opening the upcoming banners scrolls the cards rather than making the window taller, and Settings and the other views keep the same size and scroll.
- Switching a game off stops all its requests, icons included. Switching it back on checks its banners straight away.

## [0.3.0] - 2026-11-18

- Updates from GitHub, only when you choose. Once a day (or with Check now in Settings) the app looks for a newer version and shows a banner. Install update downloads it, checks it's the right file and updates in place; GachaSlop restarts by itself. Nothing is installed without you, the daily check can be switched off, and you can still download the installer yourself.

## [0.2.1] - 2026-11-18

- A Notifications list: every notification that came in is kept for 30 days. The newest unread ones show at the top of the popup; click them, or the new bell button, for the full list, grouped by day, with missed ones marked.
- Fixed the popup squashing its contents instead of scrolling.

## [0.2.0] - 2026-11-18

- Notifications are now the app's own cards above the taskbar widget (or beside the popup), instead of Windows notifications. They wait while a full-screen game is in front.
- A redesigned, easier-to-read popup: bigger text, clearer labels, and upcoming banners as a timeline.
- Zenless Zone Zero agents show their signature W-Engine.
- A Testing section in Settings to try notifications and catch-up.
- The taskbar widget's lines are spaced like the clock's.

## [0.1.0] - 2026-11-18

- First release: reset countdowns and banners for Blue Archive (Global) and Zenless Zone Zero (EU) on the taskbar and in a popup, notifications, catch-up after the PC was off, and your own games.
