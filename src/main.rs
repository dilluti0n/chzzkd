use serde::Deserialize;
use std::path::{PathBuf, Path};
use std::env;
use std::io;
use std::time::Duration;
use std::sync::Arc;
use std::fmt;
use rand::RngExt;
use log::{info, debug, warn, trace, error};

use tokio::signal::unix::{signal, SignalKind};
use tokio::task::JoinSet;

#[derive(Deserialize, Debug)]
struct PollRes {
    content: Option<PollResContent>
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "UPPERCASE")]
enum Status {
    Open,
    Close,
    #[serde(other)]
    Unknown,
}

macro_rules! define_hooks {
    (
        enum $ename:ident {
            $( $var:ident $( { $($f:tt)* } )? => $field:ident, )*
        }
        passthrough {
            $( $extra:tt )*
        }
    ) => {
        enum $ename {
            $( $var $( { $($f)* } )? , )*
            $( $extra )*
        }

        #[derive(Debug, Default, Deserialize)]
        struct Hooks {
            $( $field: Option<String>, )*
        }

        impl Hooks {
            fn script(&self, t: &$ename) -> Option<&str> {
                #[allow(unreachable_patterns)]
                match t {
                    $( $ename::$var { .. } => self.$field.as_deref(), )*
                    _ => None,
                }
            }
        }
    };
}

define_hooks! {
    enum Transition {
        WentOpen { recovered: bool } => went_open,
        WentClose => went_close,
    }
    passthrough {
        Nop { prev: Status, curr: Status },
    }
}

impl Status {
    fn transition_from(self, prev: Status) -> Transition {
        use Status::*;
        use Transition::*;

        match (prev, self) {
            (Close, Open) => WentOpen { recovered: false },
            (Unknown, Open) => WentOpen { recovered: true },
            (Open, Close) => WentClose,

            // no useful information given to user
            (_, _) => Nop { prev, curr: self },
        }
    }
}

#[derive(Deserialize, Debug)]
struct PollResContent {
    #[serde(rename = "liveTitle")]
    live_title: Option<String>,
    status: Status
}

#[derive(Deserialize)]
struct Channel {
    id: String,
    alias: Option<String>,
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.alias {
            Some(a) => write!(f, "{}", a),
            None => write!(f, "{}", self.id)
        }
    }
}

impl Channel {
    async fn fetch(&self, client: &reqwest::Client) -> Result<PollRes, reqwest::Error> {
        let id = &self.id;
        let url = format!("https://api.chzzk.naver.com/polling/v2/channels/{id}/live-status");

        client.get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }

    pub async fn event_loop(&self, client: &reqwest::Client, timeout: u64, hooks: &Hooks) {
        // Receive notifications for already opened broadcasts when first run
        let mut prev = Status::Close;
        let mut errs = 0;

        loop {
            (prev, errs) = self.tick(client, prev, errs, &hooks).await;
            let phase = jittered(timeout, errs);
            debug!("{self}: sleep for {phase:?}");
            tokio::time::sleep(phase).await;
        }
    }

    async fn tick(
        &self,
        client: &reqwest::Client, prev: Status, errs: u32, hooks: &Hooks
    ) -> (Status, u32) {
        match self.fetch(client).await {
            Ok(res) => {
                trace!("{self}: {res:?}");
                let content = match res.content {
                    Some(c) => c,
                    None => return (prev, errs)
                };

                self.event(prev, &content, hooks);

                (content.status, 0)
            }
            Err(e) => {
                let errs = errs.saturating_add(1);
                warn!("{self}: fetch failed (x{}): {e:#}", errs);

                (prev, errs)
            }
        }
    }

    fn event(&self, prev: Status, content: &PollResContent, hooks: &Hooks) {
        let tr = content.status.transition_from(prev);
        let title = content.live_title.as_deref().unwrap_or("");
        match tr {
            Transition::WentOpen { recovered } => {
                if recovered {
                    warn!("{self}: open from unknown state: {title}");
                }
                info!("{self}: WentOpen: {title}");
            },
            Transition::WentClose => {
                info!("{self}: WentClose: {title}");
            },
            Transition::Nop { prev, curr } => {
                trace!("{self}: Nop: {prev:?} => {curr:?}");
            }
        }

        if let Some(sc) = hooks.script(&tr) {
            let mut cmd = tokio::process::Command::new("/bin/sh");
            cmd.arg("-c")
                .arg(sc)
                .env("CHZZKD_ALIAS", self.to_string())
                .env("CHZZKD_LIVE_TITLE", title)
                .env("CHZZKD_ID", &self.id);

            if let Transition::WentOpen { recovered } = tr {
                cmd.env("CHZZKD_RECOVERED", if recovered { "1" } else { "0" });
            }

            // Here tokio runtime simply forks/execs and reaps it at
            // poll loop later. i.e. if the script runs in an infinite
            // loop, it will just run forever in the forked process,
            // and chzzkd cannot detect it.
            match cmd.spawn() {
                Ok(_child) => {}
                Err(e) => warn!("{self}: hook spawn failed: {e}"),
            }
        }
    }
}

#[derive(Deserialize)]
struct Config {
    #[serde(default = "default_timeout")]
    timeout: u64,
    #[serde(default)]
    hooks: Hooks,
    channel: Vec<Channel>,
}

fn default_timeout() -> u64 {
    10
}

fn jittered(timeout: u64, errs: u32) -> Duration {
    let backoff = 1u64 << errs.min(3); // 1x 2x 4x 8x
    let secs = (timeout * backoff) as f64 * rand::rng().random_range(0.9..1.1);

    Duration::from_secs_f64(secs)
}

async fn watch(cfg: Arc<Config>, idx: usize, client: reqwest::Client) {
    use tokio::time::sleep;

    let ch = &cfg.channel[idx];
    let phase = Duration::from_secs_f64(rand::rng().random_range(0.0..cfg.timeout as f64));

    sleep(phase).await;
    ch.event_loop(&client, cfg.timeout, &cfg.hooks).await;
}

/// Resolves config file path. Precedence:
///   1. argv[1]
///   2. $XDG_CONFIG_HOME/chzzkd/config.toml
///   3. $HOME/.config/chzzkd/config.toml
///   4. /etc/chzzkd/config.toml
fn resolve_cfg_path() -> Result<PathBuf, io::Error> {
    if let Some(arg) = env::args_os().nth(1) {
        let p = PathBuf::from(arg);
        return if p.is_file() {
            Ok(p)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{}: no such config file", p.display()),
            ))
        };
    }

    let xdg = env::var_os("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(|d| PathBuf::from(d).join("chzzkd/config.toml"));
    let home = env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .map(|h| PathBuf::from(h).join(".config/chzzkd/config.toml"));

    xdg.into_iter()
        .chain(home)
        .chain(std::iter::once(PathBuf::from("/etc/chzzkd/config.toml")))
        .find(|p| p.is_file())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config file found"))
}

fn load_cfg(path: &Path) -> anyhow::Result<Arc<Config>> {
    let s = std::fs::read_to_string(path)?;
    Ok(Arc::new(toml::from_str::<Config>(&s)?))
}

fn spawn_all(cfg: &Arc<Config>, client: &reqwest::Client) -> JoinSet<()> {
    let mut set = JoinSet::new();
    for i in 0..cfg.channel.len() {
        set.spawn(watch(cfg.clone(), i, client.clone()));
    }
    set
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut hup = signal(SignalKind::hangup())?;

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .build()?;

    let cfg_path = resolve_cfg_path()?;
    info!("Found config {:?}, using it", cfg_path);

    let mut cfg = load_cfg(&cfg_path)?;
    let mut tasks = spawn_all(&cfg, &client);

    loop {
        tokio::select! {
            _ = hup.recv() => {
                match load_cfg(&cfg_path) {
                    Ok(new_cfg) => {
                        info!("Received SIGHUP: reloading {} channels", new_cfg.channel.len());
                        tasks.shutdown().await;
                        cfg = new_cfg;
                        tasks = spawn_all(&cfg, &client);
                    }
                    Err(e) => {
                        warn!("Received SIGHUP: reload failed, keeping running config: {e:#}");
                    }
                }
            }
            Some(res) = tasks.join_next() => {
                match res {
                    Ok(()) => warn!("watcher exited unexpectedly"),
                    Err(e) if e.is_cancelled() => {}
                    Err(e) => error!("watcher panicked: {e}"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> Config {
        Config {
            timeout: 10,
            hooks: Hooks {
                went_open: Some("echo $CHZZKD_ALIAS: $CHZZKD_LIVE_TITLE".into()),
                ..Default::default()
            },
            channel: vec![
                Channel {
                    id: "c847a58a1599988f6154446c75366523".into(),
                    alias: Some("dopa".into())
                },
                Channel {
                    id: "a7e175625fdea5a7d98428302b7aa57f".into(),
                    alias: Some("chamcham".into())
                },
                Channel {
                    id: "6e06f5e1907f17eff543abd06cb62891".into(),
                    alias: Some("nokduro".into())
                },
                Channel {
                    id: "9381e7d6816e6d915a44a13c0195b202".into(),
                    alias: Some("lck".into())
                },
                Channel {
                    id: "0b33823ac81de48d5b78a38cdbc0ab94".into(),
                    alias: Some("wolf".into())
                },
                Channel {
                    id: "42597020c1a79fb151bd9b9beaa9779b".into(),
                    alias: Some("paka".into())
                },
                Channel {
                    id: "26ae7850ad5b6b09ca864d482dc7fa50".into(),
                    alias: Some("qb".into())
                },
                Channel {
                    id: "c100f81959d1c17044be0541eed56f5b".into(),
                    alias: Some("megajw".into())
                },
                Channel {
                    id: "b5ed5db484d04faf4d150aedd362f34b".into(),
                    alias: Some("gg".into())
                },
                Channel {
                    id: "8b3e8e3a13201cff0836c69cfab62f45".into(),
                    alias: Some("flame".into())
                },
                Channel {
                    id: "6cac96d5c9b7a9fd28903aa32fc61749".into(),
                    alias: Some("hd".into())
                },
                Channel {
                    id: "bc2dbff369307b5c446224cce192c8b1".into(),
                    alias: Some("goarosa".into())
                },
                Channel {
                    id: "732f6f16d20991243ec3f2d7afed8821".into(),
                    alias: Some("0du".into())
                },
                Channel {
                    id: "96e44e40a448971244bfd9dd8c832505".into(),
                    alias: Some("gn".into())
                },
            ],
        }
    }

    #[test]
    fn toml_parses() {
        let cfg: Config = toml::from_str(r#"
            [[channel]]
            id = "abc"
            alias = "x"

            [[channel]]
            id = "def"
        "#).unwrap();

        assert_eq!(cfg.timeout, default_timeout());
        assert!(cfg.hooks.went_open.is_none());
        assert_eq!(cfg.channel.len(), 2);
    }

    #[tokio::test]
    #[ignore] // do API request
    async fn live_tick() {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0")
            .build().unwrap();

        let cfg = sample_config();
        for ch in &cfg.channel {
            let (status, errs) = ch.tick(&client, Status::Close, 0, &cfg.hooks).await;
            assert_eq!(errs, 0, "{ch}: fetch failed");
            assert_ne!(status, Status::Unknown, "{ch}: unknown status");
        }
    }
}
