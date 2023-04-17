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

### Chat

This program also supports read-only chat via irc. It supports some basic features:

- Displays notices
- Displays users' colours
- Displays some badges (most of the colours here depend on your terminal theme):
- - Predictions
- - Sub badges are displayed as `sub/{months subbed for}`
- - Partner (verified) is displayed as a ✓ with a magenta background
- - Twitch premium is a 👑 with a blue background
- - Moderator badges are a 🗡️ with a green background
- - "Moments" badges are displayed as a 📷 with the background depending on the number of moments
- - "no audio" badges are a 🔇 with a black background
- - "no video" badges are a 👁 with a black background and strikethrough
- - "sub gifter" badges are a 🎁 with the colour depending on the number of gifted subs
- - VIP badges are a 💎 with a light magenta background

It currently lacks the following that I probably won't add, since they won't benefit me (PRs are welcome):

- Can't display arbritrary badges.
- No scrollback
- You can't log in or chat

Controls for chat are just left/right arrow keys to change tabs and `q` to quit.

## Running

```sh
$ cargo run # Optionally `--release`
```

If you don't want to see chat, you can run:

```sh
$ cargo run --no-default-features # Optionally `--release`
```

## Controls

Enjoy the following pseudocode:
```rust
match key {
  'Q' => quit,
  UpArrow | 'J' => up,
  DownArrow | 'K' => down,
  PageUp => page up,
  PageDown => page down,
  RightArrow | 'L' => match current_selection {
    Stream => play stream with streamlink,
    Game/Category => display streams under category,
    Clip => play clip with player,
    Video => play VOD with player,
  },
  LeftArrow | 'B' => go back,
  'H' => go back to home,
  '+' => increase default quality,
  '-' => decrease default quality,
  'S' | '/' => open search box, until enter key is pressed,
  'R' => refresh page,
}
```

Feel free to submit issues/PRs if you have any suggestions.
