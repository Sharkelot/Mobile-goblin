<div align="center">
  <img src="docs/art/mobile-goblin-mascot.png" alt="Mobile Goblin mascot: a green goblin in a hood and headphones" width="320">

  # Mobile Goblin

  **Your Hermes Agent, from your phone.**

  <a href="https://github.com/Sharkelot/Mobile-goblin/releases/latest/download/Mobile-Goblin.apk"><img src="https://img.shields.io/badge/Download-Android%20APK-c6ff6c?style=for-the-badge&logo=android&logoColor=17221a" alt="Download Mobile Goblin Android APK"></a>

  [![Android APK build](https://github.com/Sharkelot/Mobile-goblin/actions/workflows/apk.yml/badge.svg)](https://github.com/Sharkelot/Mobile-goblin/actions/workflows/apk.yml)
  [![Android 8.0+](https://img.shields.io/badge/Android-8.0%2B-3DDC84?logo=android&logoColor=white)](MOBILE_GOBLIN.md)
  [![License: MIT](https://img.shields.io/badge/license-MIT-33B86B)](LICENSE)
  <a href="https://slint.dev/"><img src="https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png" alt="Made with Slint" width="120"></a>
</div>

Mobile Goblin is a native Android companion for a running [Hermes Agent](https://github.com/NousResearch/hermes-agent) dashboard. It connects to the same gateway you use on your desktop, so your chats, sessions and approvals follow you to your phone. It has its own package name (`com.sharkelot.mobilegoblin`) and installs beside Kyllene.

## Install on Android

On your phone, tap **[Download Mobile Goblin APK](https://github.com/Sharkelot/Mobile-goblin/releases/latest/download/Mobile-Goblin.apk)**, then open the downloaded file and tap **Install**. If Android asks, allow your browser to install apps from this source. Android 8.0 or newer is required. The APK downloads directly; no GitHub sign-in or ZIP extraction is needed.

If you have the earlier Kotlin APK or a Rust APK from before the persistent signing key, uninstall it once before installing this build. Later builds use the same key and should install as updates.

Enter the URL of your running Hermes dashboard, plus your username and password when the gateway requires them. The phone needs a route to the gateway through your LAN, Tailscale, or HTTPS. Public HTTP sign-in is blocked.

## Current Rust build

The replacement uses a native Rust UI and Rust Hermes protocol client. It supports dashboard sign-in, live chat, recent sessions, chat rename/archive/branch, profile and model selection, reasoning and fast-tier controls, stop and steer, agent approvals and questions, masked secret prompts, automatic reconnect, cron job management, skill search and installation, toolset controls, and MCP server connection tests. A mobile menu separates chat from the management screens. It includes the goblin artwork in the interface and launcher.

The Rust APK does **not** yet support voice, phone tools, Wear, file attachments, or offline storage. Those Kotlin-era features remain in the repository as reference code but are no longer built into the APK. The Rust app does not save credentials locally; enter them again after restarting.

The GitHub release APK is debug signed with a persistent key. Chats live on the Hermes gateway. Uninstalling the app clears its local state.

## Privacy

The app sends credentials only to the Hermes dashboard address you enter. It rejects password sign-in over public HTTP. It stores no credentials or chat history on the phone and includes no analytics.

## Goblin art

| Mascot | Android launcher art |
|:---:|:---:|
| <img src="docs/art/mobile-goblin-mascot.png" alt="Full Mobile Goblin mascot" width="220"> | <img src="rust/goblin-app/resources/mipmap-hdpi/ic_launcher.png" alt="Mobile Goblin launcher artwork" width="220"> |

These are the original Mobile Goblin images. The Rust Android launcher and header use the goblin artwork.

## Build

The Android app is Rust and [Slint](https://slint.dev/). From the repository root, install Rust, the Android SDK/NDK and `cargo-apk2`, then run:

```sh
rustup target add aarch64-linux-android
cargo install cargo-apk2 --version 1.4.2 --locked
cd rust
cargo test -p goblin-core
cargo apk2 build -p mobile-goblin --target aarch64-linux-android --lib
```

The APK is produced under `rust/target/aarch64-linux-android/debug/apk/`. GitHub Actions runs the same build on every push to `main` and publishes a direct APK link on the latest release.

## Migration evidence

See [the Rust migration notes](docs/rust-migration.md), [feature coverage](docs/feature-coverage.md), and [Kyllene APK parity audit](docs/kyllene-parity-audit.md) for the shipped APK inspection, implemented scope, and remaining work.

## Upstream and license

Mobile Goblin is based on [Kyllene](https://github.com/Giton22/kyllene), which provides the Hermes gateway client and Android feature foundation. The original project is MIT licensed; its license and attribution remain in [LICENSE](LICENSE). The Rust UI uses Slint under its royalty-free mobile license; see [third-party licensing](docs/third-party-licenses.md). Mobile Goblin's branding and goblin artwork are specific to this fork. Hermes Agent is a Nous Research project; Mobile Goblin is an independent companion app.
