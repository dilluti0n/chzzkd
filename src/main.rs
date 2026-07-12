use serde::Deserialize;
use std::time::Duration;
use std::sync::Arc;
use std::fmt;
use rand::RngExt;
use log::{info, debug, warn, trace};

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

        #[derive(Debug, Default)]
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
        let title = content.live_title.as_deref().unwrap_or("None");
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
            info!("{self}: {}", sc);
        }
    }
}

struct Config {
    timeout: u64,
    hooks: Hooks,
    channel: Vec<Channel>,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .build()?;

    let cfg = Config {
        timeout: 10,
        hooks: Hooks {
            went_open: Some("echo abc".into()),
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
    };
    let cfg = Arc::new(cfg);

    let mut tasks = Vec::new();

    for i in 0..cfg.channel.len() {
        tasks.push(tokio::spawn(watch(cfg.clone(), i, client.clone())));
    }

    for t in tasks {
        t.await?;
    }

    Ok(())
}
