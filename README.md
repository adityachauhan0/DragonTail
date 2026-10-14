<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="ui/public/dragontail-logo-dark.svg">
    <img src="ui/public/dragontail-logo-transparent.svg" alt="DragonTail" width="128">
  </picture>
</p>

<h1 align="center">DragonTail</h1>

<p align="center">A Mac app for the computers on your Tailscale network, with a terminal and a remote desktop.</p>

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

Tailscale needs to be installed and logged in. The disk image is ad-hoc signed. If macOS blocks the app, right-click it and choose Open, or allow it in System Settings → Privacy & Security.

## What you can do

- See the computers on your tailnet
- Ping a computer and send it a file with Taildrop
- Open an SSH session with the `ssh` already on this Mac
- Open a remote desktop through an SSH tunnel

## Before you open it

This release is for Apple silicon Macs. A desktop session needs a VNC server already running on that computer. Passwords stay out of the app unless you save them.

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
