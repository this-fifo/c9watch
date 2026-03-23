# c9watch (personal fork)

A macOS menu bar app that monitors all running Claude Code sessions from one place. Built with Tauri 2, Rust, and SvelteKit 5.

> **This is a personal fork** of [minchenlee/c9watch](https://github.com/minchenlee/c9watch), modified to suit my own workflow. It is not intended for general use.

## Changes from upstream

- **Tailscale-only remote access** — Replaced token-based WebSocket auth with IP allowlisting for localhost and Tailscale CGNAT range. No tokens, no passwords — Tailscale handles identity and encryption.
- **No auto-updater** — Builds are done from source; the updater plugin has been removed entirely.
- **No dock icon** — Runs as a menu bar-only app (`ActivationPolicy::Accessory`).
- **No demo mode** — The demo button and sample data have been removed.
- **No marketing site** — The `website/` directory has been removed.

## Install

Prerequisites: [Rust](https://rustup.rs/) and [Node.js](https://nodejs.org/) (v18+).

```bash
./install.sh
```

This builds the app from source and installs it to `/Applications`. Pass `--no-open` to skip launching after install.

## Development

```bash
npm install
npm run tauri dev
```

## Credits

All credit for the original concept, architecture, and implementation goes to [Min-Chen Lee](https://github.com/minchenlee) and the [c9watch contributors](https://github.com/minchenlee/c9watch/graphs/contributors).

## Contributing

This fork is for personal use. Issues and pull requests will not be accepted here. If you'd like to contribute, please send changes to the [upstream repository](https://github.com/minchenlee/c9watch).

## License

MIT — see the [upstream repo](https://github.com/minchenlee/c9watch) for the original license.
