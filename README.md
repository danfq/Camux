<p align="center">
  <img src="assets/logo.png" alt="Camux logo" width="180">
</p>

<h1 align="center">Camux</h1>

<p align="center">
  Use one webcam in more than one app at the same time.
</p>

> **Camux is currently in beta.** The main experience is taking shape, but you
> may still find rough edges or features that change between releases.

## Download Camux

Go to the [latest release](https://github.com/danfq/Camux/releases/latest) and
choose the download for your computer. Each release page also lists everything
that changed since the previous version.

- **macOS:** download the `.dmg` file. It works on both Apple silicon and Intel
  Macs.
- **Linux:** download the `.deb` file for Debian or Ubuntu, or the `.rpm` file
  for Fedora and similar distributions.
- **Windows:** download the installer. The Windows version is an early preview,
  and camera support is still being developed.

Camux is not yet available through an app store, so your computer may ask you
to confirm that you trust the downloaded app when opening it for the first
time.

## What Camux does

Camux lets you choose a connected camera, preview its picture, adjust supported
camera controls, and route the video to a virtual camera that other apps can
use. This is useful when the same camera needs to appear in calls, recordings,
or streaming tools at once.

## For developers

Camux is built with Rust, Tauri, Bun, TypeScript, and React. To run it from
source, install a current Rust toolchain, Bun, and Just, then use:

```sh
git clone https://github.com/danfq/Camux.git
cd Camux
bun install --cwd core/app
just run
```

Run `just` to see the available development commands.

To publish a release, make sure the branch is clean and pushed, then run
`just release`. The release workflow chooses the next version from the latest
commit: breaking changes increase the first number, new features increase the
second, and fixes or other changes increase the third.

## License

Camux is licensed under the [GNU Affero General Public License v3.0](LICENSE).
