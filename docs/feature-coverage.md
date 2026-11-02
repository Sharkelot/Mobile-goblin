# Hermes feature coverage

Target: the running Hermes dashboard reports version 0.22.5. Its generated OpenRPC contract currently exposes more than 250 methods. The Rust client uses the dashboard protocol; this table tracks user-facing coverage rather than counting a raw RPC method as a finished feature.

| Area | Rust app today | Next work |
|---|---|---|
| Connection | Status check, basic password login, WS tickets, heartbeat, reconnect | Native OAuth and persistent signed-in state |
| Sessions | Create, list recent, resume, load recent text, new chat, stop, steer, rename, archive and branch | Search, full paging and event replay |
| Chat | Send, streaming text, current tool status | Attachments, rich tool cards, reasoning, markdown and images |
| Human input | Approvals, clarification batches, sudo and secret values, vault unlock/code | Rich multi-select UI and vault save-login |
| Agents and models | Select profile, model, reasoning effort and fast tier | Full profile management, prices, per-model capability controls |
| Automation | List, add, pause, resume and remove cron jobs | Background work status and notifications |
| Skills and tools | View installed skills, toolsets and MCP server inventory | Skill hub actions, tool configuration and MCP management |
| Voice and device | — | Voice, phone tools, Wear and push notifications |
| Local data | No credential or transcript persistence | Keystore-backed credentials, offline cache and stable release signing |

The Kotlin source remains in-tree as reference for the unported Android integrations. The Rust APK is the active build. It does not change or restart the user's running Hermes gateway.
