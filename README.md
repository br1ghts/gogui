# gtui

`gtui` is a fast terminal command center for Google Tasks, Gmail, and Google Calendar.

## Current UX Model

The app has 2 modes:

1. Command Center (default)
2. Full Module View (`Tasks`, `Gmail`, or `Calendar`)

In Command Center, all 3 tiles are interactive in place:

- Left (large): Tasks
- Top-right: Calendar agenda
- Bottom-right: Gmail inbox + preview

`Enter` on a focused tile opens that module's full view. `Esc` returns to Command Center with selection state preserved.

## Features

- Unified command-center dashboard with smooth tile focus and deterministic key routing
- Full module views for Tasks, Gmail, and Calendar
- Gmail thread preview in Command Center (includes HTML-to-text fallback)
- Quick task workflow in Command Center:
  - `a` quick add
  - `e` quick edit title
  - `x` complete/uncomplete
  - `d` delete
- Command palette (`:`) with:
  - `:help`
  - `:refresh`
  - `:goto tasks|calendar|gmail`
  - `:search <query>`
- Typed API error mapping with actionable messages for auth/scope/API issues
- Local token + cache persistence (XDG-friendly paths)

## Setup

1. Create a Google Cloud project.
2. Enable APIs:
   - Google Tasks API
   - Gmail API
   - Google Calendar API
3. Create OAuth Client ID credentials of type **Desktop app**.
4. Save credentials to:
   - `~/.config/gtui/credentials.json`
5. Run:

```bash
cargo run
```

On first run, a browser opens for OAuth consent. Tokens are then cached locally.

## OAuth Scopes

- Tasks:
  - `https://www.googleapis.com/auth/tasks`
- Gmail:
  - `https://www.googleapis.com/auth/gmail.readonly`
  - `https://www.googleapis.com/auth/gmail.modify`
  - `https://www.googleapis.com/auth/gmail.send`
- Calendar:
  - `https://www.googleapis.com/auth/calendar.readonly`
  - `https://www.googleapis.com/auth/calendar.events`

## Storage Paths

- OAuth credentials: `~/.config/gtui/credentials.json`
- Tasks token: `~/.config/gtui/token.json`
- Gmail token: `~/.config/gtui/gmail_token.json`
- Calendar token: `~/.config/gtui/calendar_token.json`
- Cache DB: `~/.local/share/gtui/cache.db`

## Keybindings

### Global

- `q` quit
- `D` toggle Command Center / Full Module View
- `Ctrl+1` Tasks full view
- `Ctrl+2` Gmail full view
- `Ctrl+3` Calendar full view
- `Tab` / `Shift+Tab` cycle focused tile (Command Center)
- `Enter` open focused tile as full module view
- `Esc` return to Command Center (or close modal/palette)
- `g` refresh active module
- `Ctrl+r` refresh all modules
- `?` help overlay
- `:` command palette

### Common Navigation

- `j` / `k` move selection in focused context
- `/` open search/filter modal

### Tasks

- `a` add task (quick in Command Center)
- `e` edit task (quick in Command Center)
- `x` toggle complete
- `d` delete task
- `c` toggle completed filter

### Gmail

- `a` archive thread
- `u` toggle unread
- `r` reply
- `c` compose
- `/` search threads

### Calendar

- `n` new event
- `e` edit event
- `d` delete event
- `t` jump to today
- `[` / `]` move date range backward/forward
- `/` search events

## Error Handling / 403 Prevention

The app maps Google API failures into typed errors and shows actionable status messages:

- `401` -> auth expired
- `403 accessNotConfigured` -> enable API in Google Cloud Console
- `403 insufficientPermissions` -> re-auth required for missing scope
- `429` -> rate limited
- `5xx` -> transient server error

Example actionable messages:

- `Enable Google Calendar API in Google Cloud Console`
- `Re-auth required: missing scope calendar.events`

If scopes changed, delete the relevant token file and re-run for OAuth again.

## Development

```bash
cargo check
cargo test
```

If installed in your toolchain:

```bash
cargo fmt --check
cargo clippy -- -D warnings
```

## Roadmap / Future Ideas

- Better HTML email rendering (quotes/signatures collapsing, richer entity handling)
- Scrollable Gmail preview pane in Command Center for long messages
- Natural language quick-add for Calendar and Tasks in command palette
- Smarter Calendar grouping and timezone controls
- Better offline cache browsing and stale-data indicators
- Configurable themes/keymaps
- Optional notifications/reminders panel
