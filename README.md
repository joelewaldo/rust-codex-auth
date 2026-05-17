# codex-auth

Minimal CLI for switching between Codex ChatGPT OAuth accounts.

## Install

Download the archive for your platform from the [latest release](https://github.com/joelewaldo/rust-codex-auth/releases/latest), extract the binary, and place it somewhere on your `PATH`.

### macOS

Apple Silicon:

```bash
curl -LO https://github.com/joelewaldo/rust-codex-auth/releases/latest/download/codex-auth-aarch64-apple-darwin.tar.gz
tar -xzf codex-auth-aarch64-apple-darwin.tar.gz
mkdir -p "$HOME/.local/bin"
install -m 0755 codex-auth-aarch64-apple-darwin/codex-auth "$HOME/.local/bin/codex-auth"
```

Intel:

```bash
curl -LO https://github.com/joelewaldo/rust-codex-auth/releases/latest/download/codex-auth-x86_64-apple-darwin.tar.gz
tar -xzf codex-auth-x86_64-apple-darwin.tar.gz
mkdir -p "$HOME/.local/bin"
install -m 0755 codex-auth-x86_64-apple-darwin/codex-auth "$HOME/.local/bin/codex-auth"
```

If `~/.local/bin` is not already on your `PATH`, add this to `~/.zshrc`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Then restart your shell or run:

```bash
source ~/.zshrc
```

### Linux

```bash
curl -LO https://github.com/joelewaldo/rust-codex-auth/releases/latest/download/codex-auth-x86_64-unknown-linux-gnu.tar.gz
tar -xzf codex-auth-x86_64-unknown-linux-gnu.tar.gz
mkdir -p "$HOME/.local/bin"
install -m 0755 codex-auth-x86_64-unknown-linux-gnu/codex-auth "$HOME/.local/bin/codex-auth"
```

If `~/.local/bin` is not already on your `PATH`, add this to `~/.profile`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Then restart your shell or run:

```bash
. ~/.profile
```

### Windows

```powershell
Invoke-WebRequest -Uri "https://github.com/joelewaldo/rust-codex-auth/releases/latest/download/codex-auth-x86_64-pc-windows-msvc.zip" -OutFile "codex-auth.zip"
Expand-Archive -Path "codex-auth.zip" -DestinationPath "." -Force
New-Item -ItemType Directory -Force -Path "$HOME\bin" | Out-Null
Copy-Item ".\codex-auth-x86_64-pc-windows-msvc\codex-auth.exe" "$HOME\bin\codex-auth.exe"
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$HOME\bin*") {
  $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) { "$HOME\bin" } else { "$userPath;$HOME\bin" }
  [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
}
```

Open a new terminal after updating the user `PATH`.

Verify the install with:

```bash
codex-auth --help
```

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

## Releases

Releases are created from semantic version tags such as `v0.1.0`. To publish one:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow builds Linux x64, macOS Intel, macOS Apple Silicon, and Windows x64 archives, publishes them to the GitHub Release, and uploads `SHA256SUMS` for verification.
