# MovieBox-Tui

A lightning fast, zero-config terminal user interface (TUI) for streaming movies and TV series directly from your terminal.

## Installation

### macOS (recommended)

The installer checks and installs `pkgconf`, `chafa`, and `mpv`, builds the app,
and makes sure Cargo's binary directory is available in your shell:

```sh
./scripts/install-macos.sh
```

This avoids the `Failed to find chafa via pkg-config` build error seen when
`ratatui-image` is compiled without the native Chafa library.

### Manual

```sh
brew install pkgconf chafa mpv
cargo install --path . --locked
```

Requires Rust 1.90 or newer. Other platforms must provide `pkg-config`, Chafa
1.8 or newer, and `mpv` through their system package manager.

## Turkish subtitles

MovieBox subtitles are checked first. Turkish captions (`Türkçe`, `Turkish`,
`tr`, or `tur`) are moved to the top and selected by default. If MovieBox does
not provide Turkish subtitles, the app checks its local cache, searches SubDL,
then uses OpenSubtitles as a final fallback. Downloaded subtitles are cached, so
replaying the same movie or episode does not consume another provider download.

Create a free key in the [SubDL API panel](https://subdl.com/panel/api), then
either run the macOS installer and paste the key when prompted, or set:

```sh
export SUBDL_API_KEY="your-api-key"
```

The installer stores it at `~/.config/moviebox-tui/subdl_api_key`. SubDL is the
recommended provider because its free plan has a substantially higher quota.

The OpenSubtitles REST API requires an API key. Create one in your
[OpenSubtitles consumer account](https://www.opensubtitles.com/en/consumers),
then either run the macOS installer and paste the key when prompted, or set:

```sh
export OPENSUBTITLES_API_KEY="your-api-key"
```

The installer stores a provided key at
`~/.config/moviebox-tui/opensubtitles_api_key` with user-only permissions. API
keys are never committed to the repository. Cached subtitle files are stored in
the operating system's user cache directory under `moviebox-tui/subtitles`.

## Usage

Launch the app from your terminal:

```sh
moviebox
```

- **Search**: Press `/` to search for movies or shows.
- **Play**: Select a result and press `Enter` to stream instantly.
- **Logs**: Press `Ctrl+L` to view internal network logs.
- **Quit**: Press `q` or `Esc` to exit.

## Features

- Instant streaming with `mpv`
- Full metadata (seasons, episodes, dubs, and subs)
- Turkish subtitles preferred automatically, with SubDL and OpenSubtitles fallback
- Built in geo-unblocking (zero VPN required)
- Copy direct stream URLs to clipboard

## Screenshots

<details>
<summary>Click to view screenshots</summary>

<br>

### Home Screen
<img src="assets/screenshots/1-home.jpg" alt="Home Screen" width="800">

### Search Results
<img src="assets/screenshots/2-search.jpg" alt="Search Results" width="800">

### Movie Details
<img src="assets/screenshots/3-movie.jpg" alt="Movie Details" width="800">

### Stream Selection
<img src="assets/screenshots/4-streams.jpg" alt="Stream Selection" width="800">

### TV Series Details
<img src="assets/screenshots/5-series.jpg" alt="TV Series Details" width="800">

</details>

## License

Dual-licensed under MIT or Apache-2.0.
