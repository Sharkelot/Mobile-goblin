# Rust migration evidence and scope

## Observed

- The prior shipped `androidApp-debug.apk` was inspected with REA `inspect-apk-signature`. It has an APK v2 signature and one signer. SHA-256: `545234cc5e365ec078c045c9c349a97550da93bb85a03613ff9800e491d9c52d`. This establishes the artifact identity and signing structure, not behavioral equivalence.
- The source tree contains a Kotlin Multiplatform Hermes client and Android UI. The Android source and `docs/hermes-protocol-research.md` were used to map dashboard authentication, WebSocket handshake, JSON-RPC frames, and chat calls.
- REA package decompilation could not run in this environment because its JADX provider JAR is not configured. The Rust port therefore uses source evidence rather than claiming decompiled behavior.

## Implemented in Rust

- `goblin-core`: URL normalization, private-network classification, REST status/password login/WS tickets, profile-scoped session reads, JSON-RPC framing, typed human-input requests, gateway URL credential rejection, and tests.
- `goblin-app`: native Slint Android UI, goblin art, sign-in, ticketed WebSocket, create/send/new-chat, recent-session browsing and resume, stored transcript display, reply streaming, profile/model selection, reasoning and fast-tier controls, stop and steer, approvals, clarification and secret prompts, heartbeat checks, reconnection and session resume, and visible connection/error state.
- Android package ID remains `com.sharkelot.mobilegoblin`; the GitHub APK workflow now builds the Rust app.

## Remaining parity work

Attachments, voice, phone tools, Wear, offline persistence, full settings and tool management surfaces, and persistent release signing remain to be ported. The Kotlin implementation remains in-tree as reference material. A successful Rust build does not establish end-to-end Hermes compatibility until tested against a live gateway and on Android hardware.
