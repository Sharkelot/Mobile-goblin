# Kyllene parity audit

This audit compares the shipped Kyllene 0.14.0 APK with the signed Mobile Goblin Rust APK built from `ccb7f65`. The comparison uses Reverse Engineer Anything 6.8.0, its JADX 0.7.1 Android provider, and the Kotlin source retained in this repository. Static package evidence does not prove runtime behavior; device interaction still needs an Android test.

## Shipped package evidence

| Observation | Kyllene 0.14.0 | Mobile Goblin Rust |
|---|---|---|
| APK SHA-256 | `eaa4d7f8fe98275fbd28284077408161c62e2364c4a70a7a6932faa02d0e1a59` | `41fed8f035bffa870706cbfa4e861e84505c97dc56609c5d0c8e192aca24b371` |
| Package | `dev.herald.android` | `com.sharkelot.mobilegoblin` |
| Android permissions | 26, including audio, notifications, location, calendar, contacts, health and biometrics | 1: internet |
| Declared services | 8, including foreground chat, voice call, Wear bridge and assist | 0 |
| Declared receivers | 5, including restart, notification actions and geofencing | 0 |
| Offline database | `dev.hermeskotlin.core.cache.OfflineDatabase` is present in the APK | None |
| Signing | APK signature schemes v2 and v3 | Signed APK; verified by the build and REA |

REA identified Kyllene's `ChatService.onStartCommand` as starting a foreground notification. It also identified a `WearBridgeService`, `VoiceCallService`, `HeraldAssistService`, and `GeofenceReceiver` in the manifest. These are shipped Android integrations, not just source files.

## User-facing comparison

The Kotlin source has a gateway chooser and sign-in route, chat with a sessions sidebar, bot roster, rooms, inbox, scheduled jobs, insights, capabilities, plugins, settings, voice screens, media and attachment views, rich tool events, and phone and Wear integrations. Mobile Goblin currently has a live chat, a full-screen chat browser with recent and archived listings and gateway-backed search, profile and model controls, human input responses, cron/skills/toolset/MCP controls, and basic chat management. Its menu separates chat from management screens, but it is not yet equivalent to the Kyllene navigation or feature set.

The largest missing blocks are:

1. **Conversation UI:** message, reasoning, notice, and basic tool rows are now separate; rich expandable tool cards, markdown, media, attachments, voice, find-in-chat, and deep history paging remain.
2. **Android runtime:** foreground chat and voice services, notifications/actions, phone tools, Wear, assist, and background recovery.
3. **App data and navigation:** multiple gateways, bot roster, rooms, inbox, settings, offline cache, and secure credential persistence.

Do not label a raw gateway RPC button as feature parity for a missing Android integration. Restore each block with an actual screen, gateway behavior, Android lifecycle handling, and device test before marking it complete.

## Reproduce the evidence

The upstream APK is [Kyllene 0.14.0](https://github.com/Giton22/kyllene/releases/tag/v0.14.0). The provider is [jadx-headless-mcp 0.7.1](https://github.com/1013503897/jadx-headless-mcp/releases/tag/v0.7.1), SHA-256 `6e5eacf500b64292bfb73c49797c1958f6ee44646e43e868039ae7feb573ff75`.

Run `rea inspect-artifact`, `rea inspect-apk-signature`, and `rea inspect-android-package` on both APKs. With `REA_JADX_MCP_JAR` pointing to the verified provider JAR and Java 17+, run `rea inspect-android-class` for `dev.hermeskotlin.android.notify.ChatService` and `rea inspect-android-method` for its `onStartCommand`. The REA evidence IDs for this audit are `ev_cdab591545e1fa38e1836bfc02cc862ab437ec288207edee2f699d262dc59555` (upstream package), `ev_6c9ca22886ad463cf7b71e66e1687c799efbb2aed6e1f010623599790eb5cd25` (Rust package), and `ev_0d6651f6a47cbaceb8fa56d972c8b940dc2980392c800779656fe82259d3b451` (foreground service method).
