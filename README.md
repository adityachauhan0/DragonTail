# DragonTail

DragonTail is a macOS app for the computers on your Tailscale tailnet. It lists devices, pings them, sends files with Taildrop, opens an SSH terminal, and shows a remote desktop through an SSH tunnel.

DragonTail is an independent client. It is not affiliated with Tailscale. The project site is [dragontail.site](https://dragontail.site).

## What it does

- Shows the live tailnet from `tailscale status`
- Pings a peer and runs a network check on this Mac
- Sends a file with `tailscale file cp`
- Opens a real SSH session in the window, using the `ssh` on this Mac
- Connects to a VNC server on a peer, through an SSH tunnel, and draws it with noVNC

A peer needs Tailscale, and a desktop session needs a VNC server already running on that computer.

## Requirements

The 0.1.0 build is for Apple silicon Macs. Tailscale needs to be installed and logged in.

To build from source you also need Node.js, npm, and Rust.

## Develop

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

The macOS app and disk image are written under `target/release/bundle/`.

## License

[MIT](LICENSE). Commit Mono, used for the terminal, is under the SIL Open Font License in `ui/public/fonts/OFL.txt`.
