# chzzkd
Stream notification daemon for <https://chzzk.naver.com>. It executes
shell hooks (by `/bin/sh -c`) when listed channels go live or
offline. Useful context, such as the stream title, is passed to the
script as environment variables.

Example configuration:
```toml
timeout = 10

[hooks]
went_open = '''
url="https://chzzk.naver.com/live/$CHZZKD_ID"
action=$(notify-send -a chzzkd \
  -A default="mpv" \
  "$CHZZKD_ALIAS: hello" \
  "$CHZZKD_LIVE_TITLE")

case "$action" in
  default) mpv "$url" ;;
esac
'''
went_close = 'notify-send -a chzzkd "$CHZZKD_ALIAS: bye"'

[[channel]]
id = "c847a58a1599988f6154446c75366523"
alias = "dopa"

[[channel]]
id = "a7e175625fdea5a7d98428302b7aa57f"
alias = "chamcham"

[[channel]]
id = "6e06f5e1907f17eff543abd06cb62891"
alias = "nokduro"

[[channel]]
id = "c100f81959d1c17044be0541eed56f5b"
alias = "megajw"

[[channel]]
id = "8b3e8e3a13201cff0836c69cfab62f45"
alias = "flame"

[[channel]]
id = "732f6f16d20991243ec3f2d7afed8821"
alias = "0du"
```

This polls each channel every 10 seconds. When a stream goes live
(`went_open`), it sends a clickable desktop notification. Clicking it
opens the stream directly in `mpv`. When a stream goes offline
(`went_close`), it says goodbye and leaves.

Also, you may find it useful that
```sh
chromium --app="https://chzzk.naver.com/live/$CHZZKD_ID/chat"
```
opens the popup chat window for the channel. :)

---

How to install and run:
```sh
git clone https://github.com/dilluti0n/chzzkd
cd chzzkd
cargo install --path .
mkdir -p "$HOME/.config/chzzkd/"
cp example_config.toml "$HOME/.config/chzzkd/config.toml"
chzzkd

# Run as daemon
RUST_LOG=info setsid chzzkd 2>chzzkd.log >/dev/null

# Stop daemon
pkill chzzkd
```

Synopsis: `chzzkd [CONFIG]`

Configuration path precedence:
  1. `CONFIG` (command line argument)
  2. $XDG_CONFIG_HOME/chzzkd/config.toml
  3. $HOME/.config/chzzkd/config.toml
  4. /etc/chzzkd/config.toml
