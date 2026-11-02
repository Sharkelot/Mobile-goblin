# Mobile Goblin

Mobile Goblin is an Android companion for a running Hermes Agent dashboard. It is based on the MIT-licensed [Kyllene](https://github.com/Giton22/kyllene) project, with separate Android identity, original goblin art, and green branding. The original license remains in `LICENSE`.

## Install

1. Open the repository's [Android APK workflow](https://github.com/Sharkelot/Mobile-goblin/actions/workflows/apk.yml), select the latest successful run, and download `mobile-goblin-apk` under **Artifacts**.
2. Unzip it and open `androidApp-debug.apk` on your Android phone. Allow installation from your file manager if Android asks. Android 8.0 or later is required.
3. On the first screen, enter the same Hermes dashboard address you use from the desktop, including the port when needed. Use a Tailscale address or HTTPS endpoint when away from home. Tap **Test connection**, then sign in with your Hermes dashboard account.

With Android platform tools, installation is `adb install -r androidApp-debug.apk`. The package is `com.sharkelot.mobilegoblin`, so it can coexist with Kyllene. A new debug APK from this workflow installs over an earlier debug APK of this fork. Production release builds need a persistent release signing key to support upgrades reliably.

## Desktop to mobile behavior

The phone and desktop talk to the **same Hermes gateway**. Chats and session state come from that gateway; Mobile Goblin is not paired directly with a desktop app. The app uses the dashboard status probe, authenticated gateway socket, session and approval APIs, and reconnection logic inherited from Kyllene. The desktop must keep the Hermes dashboard running and the phone must be able to reach it. Do not put the dashboard's login credentials or private URL in this repository.

The phone app offers streaming chats, session switching, approval prompts, attachments, voice, notifications and optional phone tools. Phone tool permissions are requested on the device as the user enables each feature. Gateway plugins offered inside the app are separate server-side installations; review them before enabling them.

## Build locally

Install Android SDK with API 37.1, a compatible JDK 25, and the Android build tools. Then run:

```sh
./gradlew :androidApp:assembleDebug
```

The APK is written to `androidApp/build/outputs/apk/debug/androidApp-debug.apk`.

## Artwork

The [full mascot](docs/art/mobile-goblin-mascot.png) and [launcher artwork](androidApp/src/main/res/drawable-nodpi/mobile_goblin_launcher.png) are included in this repository. Adaptive launcher icons use the goblin image on different backgrounds. Themed icons and notifications use a goblin silhouette, and the chat avatar uses a matching vector mark.
