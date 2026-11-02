# Kyllene 0.14.0 → Rust Mobile Goblin parity checklist

This is the working inventory for replacing the shipped Kyllene Android app with the Rust APK. It is based on Kyllene's retained Kotlin source (`shared/core`, `shared/ui`, `androidApp`, `wearApp`) and the shipped APK evidence in [the parity audit](kyllene-parity-audit.md). A feature counts as **done** only when the Rust app exposes the same user action, the gateway or Android behavior works, errors and lifecycle states are handled, and a relevant test or device run verifies it. A successful RPC call or a static APK inspection alone does not finish a feature.

Status: **D** = verified slice, **P** = some behavior exists, **M** = missing. Status describes the Rust code in this change; Android publication and connected UI validation follow in CI. The Kotlin app remains the behavioral reference, not the release target. Work through the rows in order of dependencies; credentials and storage must precede background services, and media transport must precede voice and share targets.

| ID | Area and completion target | Status |
|---|---|:---:|
| G01 | Add and switch among saved gateways, with gateway names, reachability, and removal | P |
| G02 | Password sign-in, errors, session expiry and reauthentication | P |
| G03 | Browser/OAuth sign-in return and provider selection | M |
| G04 | Persist credentials/session securely with Android Keystore and restore on launch | M |
| G05 | WebSocket ticket, heartbeat, reconnect and state recovery after network change | P |
| G06 | Support private LAN/Tailnet HTTP and public HTTPS with clear transport errors | P |
| N01 | Chats, Bots, Rooms, Inbox, Scheduled, Archived, Capabilities, Config, Plugins and Insights navigation | P |
| N02 | Back stack, deep links, drawer/search, keyboard focus and small-screen scrolling | P |
| N03 | `hermes://` new chat, voice chat, session and bot links; launcher shortcuts | M |
| N04 | Android share targets for text, URLs, images and files | M |
| C01 | New chat, send, stream text/reasoning, stop and steer | P |
| C02 | Load full history with paging, replay, compacted turns and continuity after reconnect | P |
| C03 | Separate user, assistant, tool, notice, error and human-input events in order | P |
| C04 | Render Markdown, code, links, tables and copy/select actions | M |
| C05 | Expand tool calls/results, status, errors and artifacts | P |
| C06 | Composer drafts, multiline editing, slash commands and suggestions | M |
| C07 | Attach photos, camera captures, arbitrary files and media with upload progress | M |
| C08 | Render and download images, audio, video and document attachments | M |
| C09 | Find in chat, jump to result and message context actions | M |
| C10 | Approval choices, clarification batches, sudo, vault unlock and secrets with correct masking | P |
| C11 | Session rename, archive/unarchive, branch and delete where Kyllene offers them | P |
| C12 | Profile, model, reasoning and fast-tier selection, including cost confirmation | P |
| S01 | Recent/archived chat lists, search, filters and deep paging | P |
| S02 | Session previews, unread/running state, ordering and resume | P |
| S03 | Project grouping/list view and project indexes for chats | M |
| S04 | Thread navigation and linked conversation context | M |
| B01 | Bot roster, pinned/hidden order, previews and canonical chat opening | P |
| B02 | Create, edit, delete, pin and hide bots; avatar and start instructions | M |
| B03 | Bot health, routine status, presence and profile-specific options | M |
| R01 | Hosted and Desktop room lists, logs, members and unread state | P |
| R02 | Room text sending with ack, replay, error recovery and event identity | P |
| R03 | Create, configure, stop and leave rooms; member and approval controls | M |
| R04 | Room attachments, media and deep log paging | M |
| I01 | Register phone with herald-push; receive and recover run notices | M |
| I02 | Inbox run list, report details, failures, unread state and pagination | M |
| I03 | Inbox follow choices, job/group folders, layout, rename and mark-read actions | M |
| I04 | Continue a run's chat or ask about a report with the full attachment | M |
| J01 | List, add, pause, resume and remove scheduled jobs | P |
| J02 | Full schedule/job editor, validation, timezone and target profile | M |
| J03 | Job runs/history, failure state and notification links | M |
| A01 | Usage totals for 7/30/90 days, costs, tokens and breakdowns | P |
| A02 | Daily charts, filtering, detail drill-down and display parity | M |
| K01 | Skills, Toolsets and MCP lists with profile-aware enable switches | P |
| K02 | Skill hub search/browse/inspect/install and update/remove flows | P |
| K03 | Toolset configuration, availability/setup and per-tool controls | M |
| K04 | MCP add/edit/remove, transport/credentials and connection diagnostics | M |
| F01 | Config/settings editor, model/provider prices and capability controls | M |
| F02 | Plugin page discovery per profile with metadata and hidden-tab filtering | P |
| F03 | Open a plugin page in-app with authenticated gateway content | M |
| F04 | Plugin install/enable/update/remove and Next Prompt suggestion lifecycle | M |
| F05 | Suggested replies, link previews and chat starting instruction | M |
| V01 | Voice capture, playback, permissions and background audio lifecycle | M |
| V02 | Realtime voice call/WebRTC, interruption and route changes | M |
| V03 | Dictation/speech-recognition service and voice deep links | M |
| P01 | Phone tools settings, consent, capability detection and user-visible logs | M |
| P02 | Calendar, contacts, location and watched places/geofencing | M |
| P03 | Health Connect steps, sleep and heart rate with permission handling | M |
| P04 | Sound/DND, alarms/timers, screen time and open/share actions | M |
| P05 | Assistant overlay, recognition and phone action approval UI | M |
| W01 | Wear app pairing, message/channel bridge and chat actions | M |
| W02 | Watch notifications, voice and disconnected recovery | M |
| L01 | Foreground chat and microphone services with accurate notifications | M |
| L02 | Notification actions, full-screen call UI and Android 13+ permission flow | M |
| L03 | Boot/package-replaced recovery, network/offline transitions and background reconnect | M |
| L04 | Offline database for transcripts, drafts, inbox and last known state | M |
| L05 | Settings persistence: theme/accent, pure black, glow, text size, wallpaper/strength and app icons | M |
| L06 | Android lifecycle rotation, process death, lock screen and battery restrictions | M |
| L07 | Chat display settings: reasoning/tools/tokens/timestamps/code wrap, pet and link previews | M |
| L08 | App lock, notification preferences, account controls and update checking | M |
| T01 | Rust protocol tests against Kyllene's actual HTTP/RPC routes | P |
| T02 | Emulator install, launch, login keyboard and connected gateway UI tests | P |
| T03 | Physical arm64 install/upgrade, login, chat, background and notification tests | M |
| T04 | Shipped APK manifest/signature/component comparison with REA after each integration milestone | P |
| T05 | Reproducible signed release, direct APK download and upgrade without data loss | P |

## Current order of work

1. Finish the read-only Plugins catalogue (`F02`), then build secure persisted login (`G04`); an active reconnect now reuses its cookie and the app remembers the last gateway address, but an app restart still requires sign-in.
2. Add authenticated in-app plugin pages (`F03`) and a local data layer (`L04`), which unlock reliable background behavior.
3. Complete core chat/media and session flows (`C02`–`C09`, `S01`–`S02`).
4. Complete management screens and Inbox (`B02`–`B03`, `R03`–`R04`, `I01`–`I04`, `J02`–`J03`, `K02`–`K04`, `F01`, `F04`).
5. Add Android services, notifications, phone, voice and Wear integrations (`L01`–`L06`, `P01`–`P05`, `V01`–`V03`, `W01`–`W02`).
6. Recheck all rows against Kyllene's source and APK, then run connected emulator and physical device acceptance (`T01`–`T05`).

No live Hermes gateway is restarted by this plan. Tests use a stub or a separate test session until device acceptance is possible.
