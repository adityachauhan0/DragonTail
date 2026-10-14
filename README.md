<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="ui/public/dragontail-logo-dark.svg">
    <img src="ui/public/dragontail-logo-transparent.svg" alt="DragonTail" width="128">
  </picture>
</p>

<h1 align="center">DragonTail</h1>

<p align="center">DragonTail is a Mac app to set up VNC on your Tailscale network. The viewer is included. This Mac needs Tailscale, logged in. The other computer needs to be on that tailnet, with a VNC server listening on a port.</p>

<p align="center">
  <a href="https://github.com/adityachauhan0/DragonTail/releases/download/v0.1.0/DragonTail_0.1.0_aarch64.dmg"><strong>Download for Mac</strong></a>
  &nbsp;·&nbsp;
  <a href="https://dragontail.site">Site</a>
  &nbsp;·&nbsp;
  <a href="LICENSE">MIT</a>
</p>

<p align="center">DragonTail is not made by Tailscale.</p>

<p align="center">
  <img src="docs/demo.gif" alt="DragonTail: onboarding, a live desktop, Taildrop, and a ping" width="1128">
</p>

<p align="center">Onboarding, a live desktop, Taildrop, and a ping.</p>

## Download

DragonTail 0.1.0 is an Apple silicon build.

[DragonTail_0.1.0_aarch64.dmg](https://github.com/adityachauhan0/DragonTail/releases/download/v0.1.0/DragonTail_0.1.0_aarch64.dmg)

The disk image is ad-hoc signed. If macOS blocks the app, right-click it and choose Open, or allow it in System Settings → Privacy & Security.

## Prerequisites

- Tailscale is installed on this Mac, and you are logged in.
- The computer you want to view is on that tailnet, with a VNC server listening on a port.
- DragonTail includes the viewer, and draws the desktop in the app.

## Set up VNC on Tailscale

1. Log in to Tailscale on this Mac.
2. Open DragonTail.
3. Open the computer whose VNC port is already listening.

## What you can do

- See the computers on your tailnet
- Ping a computer and send it a file with Taildrop
- Open an SSH session with the `ssh` already on this Mac
- Open that computer's desktop in DragonTail

## Build

Node.js, npm, and Rust are required.

```bash
npm install
npm run tauri dev
```

```bash
npm run tauri build
```

The app and the disk image are written under `target/release/bundle/`.

## License

[MIT](LICENSE). Commit Mono, the terminal face, is under the SIL Open Font License in [`ui/public/fonts/OFL.txt`](ui/public/fonts/OFL.txt).
