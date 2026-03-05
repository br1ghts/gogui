# gtui

`gtui` is a full-screen terminal UI for Google Tasks, Gmail, and Calendar.

## Setup

1. Create a Google Cloud project.
2. Enable these APIs in the same project:
   - Google Tasks API
   - Gmail API
   - Google Calendar API
3. Create OAuth Client ID credentials of type **Desktop app**.
4. Put credentials at:
   - `~/.config/gtui/credentials.json`
5. Run:

```bash
cargo run
```

## Tokens and Cache

- Tasks token: `~/.config/gtui/token.json`
- Gmail token: `~/.config/gtui/gmail_token.json`
- Calendar token: `~/.config/gtui/calendar_token.json`
- Cache DB: `~/.local/share/gtui/cache.db`

## Scopes

- Tasks: `https://www.googleapis.com/auth/tasks`
- Gmail:
  - `https://www.googleapis.com/auth/gmail.readonly`
  - `https://www.googleapis.com/auth/gmail.modify`
  - `https://www.googleapis.com/auth/gmail.send`
- Calendar:
  - `https://www.googleapis.com/auth/calendar.readonly`
  - `https://www.googleapis.com/auth/calendar.events`

## Calendar 403 Prevention

The app maps Google API errors into typed statuses and shows actionable UI messages.

- `accessNotConfigured` -> `Enable Google Calendar API in Google Cloud Console`
- `insufficientPermissions` -> `Re-auth required: missing scope calendar.events`

If scopes changed, delete `~/.config/gtui/calendar_token.json` and re-run to re-auth.

## Keybindings

- Global: `q` quit, `T` switch tab, `tab` next pane, `h/l` pane nav, `j/k` selection, `g` refresh, `?` help
- Calendar: `n` new event, `e` edit, `d` delete, `t` today, `[`/`]` shift 14-day range, `/` search

## Checks

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```
