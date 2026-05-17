# codex-auth

Minimal CLI for switching between Codex ChatGPT OAuth accounts.

## Commands

```text
codex-auth login [--device-auth]
codex-auth list
codex-auth switch [row|email]
codex-auth remove <row|email>
```

`login` delegates to `codex login`, then stores the resulting `auth.json` snapshot under:

```text
<CODEX_HOME>/rust-codex-auth/accounts/
```

The tool also keeps a small usage registry at:

```text
<CODEX_HOME>/rust-codex-auth/registry.json
```

`list` shows remaining 5-hour and weekly limits plus relative reset times. Fresh local rollout usage is attributed to the active account and stored in the registry, so the value shown for the current account matches Codex's own local usage state when available. Every other row is refreshed from the remote usage endpoint at most once per minute with bounded concurrency, and successful responses update the cached registry snapshot. Repeated `list` calls in the same minute reuse that cache. `switch` accepts either a row number or an exact email; when an email belongs to multiple workspaces, use the row number shown by `list`. Bare `switch` prints the cached list and prompts for a row number.

## Network behavior

For rows refreshed remotely, `list` sends the saved account's access token to OpenAI's ChatGPT usage endpoint:

```text
GET https://chatgpt.com/backend-api/wham/usage
```

If the request fails for one row, the command shows the row-level failure instead of hiding it.
