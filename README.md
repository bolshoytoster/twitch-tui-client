# Twitch client in the terminal

Based on [this python program](https://gitlab.com/corbie/twitch-curses), but uses the current twitch API,
doesn't use ncurses (bleh) and is written in rust ([wooo](https://youtu.be/sAXZbfLzJUg)). The UI is
massively based on that.

I originally tried to just rewrite the networking bits of the python program to work with the current
twitch API, but then I realised that I hate weak typing, so I completely rewrote it.

Still uses `streamlink` for streams, but can alsp use your specified player (`ffplay` by default (you can
set it at the top of `src/config.rs`)) for clips and VODs.

I did this instead of revising.

## Features

- Very configurable (see `src/config.rs`)
- Displays broadcaster colours where possible
- Many varied pages (any of these can also be your home page):
- - `Shelves`: main home page
- - `PersonalSection`: the bit on the left on the webapp
- - Game: categories (the API refers to them as games)
- - Search: you know
- You can watch streams, clips and VODs at any quality

Feel free to submit issues/PRs if you have any suggestions.
