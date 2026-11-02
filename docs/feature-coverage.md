# Hermes feature coverage

Target: the running Hermes dashboard reports version 0.22.5. Its generated OpenRPC contract currently exposes more than 250 methods. The Rust client uses the dashboard protocol; this table tracks user-facing coverage rather than counting a raw RPC method as a finished feature.

| Area | Rust app today | Next work |
|---|---|---|
| Connection | Status check, basic password login, WS tickets, heartbeat, reconnect | Native OAuth and persistent signed-in state |
| Sessions | Create, browse recent and archived, search stored chats, resume, load recent text, new chat, stop, steer, rename, archive and branch | Full paging and event replay |
| Bots | Browse the gateway's profile roster, including pinned/hidden order and chat previews; open existing canonical Bot Chats | Create/edit bots, start missing canonical chats safely, pin/hide, avatars, health and routine status |
| Rooms | Browse hosted rooms and recent gateway logs, send text with a durable in-process event ID and log confirmation, poll active rooms, and read Desktop room mirrors with tombstone handling | Hosted room create, stop, approvals, attachments and deep log paging |
| Chat | Send, streaming text and reasoning, separate user/assistant/tool/notice rows, tool summaries matched by gateway ID, stored text and image placeholders | Attachment viewing, expandable tool details, markdown, images, and full event replay |
| Human input | Approvals, clarification batches, sudo and secret values, vault unlock/code | Rich multi-select UI and vault save-login |
| Agents and models | Select profile, model, reasoning effort and fast tier | Full profile management, prices, per-model capability controls |
| Automation | List, add, pause, resume and remove cron jobs | Background work status and notifications |
| Insights | Profile-aware gateway usage for 7, 30 and 90 days, including cost, tokens, model, tool and skill breakdowns | Interactive daily chart and richer presentation |
| Capabilities | Profile-aware Skills, Toolsets and MCP lists with per-item enabled state and REST switches; plugin-owned MCP servers and unconfigured toolsets are identified | Full editor, setup flows and server management |
| Skills and tools | View installed skills and toolsets; skill hub search, browse, inspect and install; enable/disable toolsets; list and test MCP servers | Full MCP server management and richer tool configuration |
| Voice and device | — | Voice, phone tools, Wear and push notifications |
| Local data | No credential or transcript persistence | Keystore-backed credentials, offline cache and stable release signing |

The Kotlin source remains in-tree as reference for the unported Android integrations. The Rust APK is the active build. Capability list and switch requests run off the live chat socket task, so they do not stall chat streaming or heartbeat. It does not change or restart the user's running Hermes gateway unless the user taps a capability switch, which saves that profile setting.

See the [Kyllene shipped APK parity audit](kyllene-parity-audit.md) for the Android component gap and evidence behind this roadmap.
