use serde;
use serde::Deserialize;
use std::time::Duration;
use std::sync::Arc;
use std::fmt;
use rand::RngExt;
use log::{debug, warn, trace};

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

    pub async fn event_loop(&self, client: &reqwest::Client, timeout: u64) {
        use tokio::time::sleep;

        let name = self.to_string();
        let mut errs = 0u32;

        loop {
            match self.fetch(&client).await {
                Ok(res) => {
                    trace!("{name}: {res:?}");
                }
                Err(e) => {
                    errs = errs.saturating_add(1);
                    warn!("{name}: fetch failed (x{errs}): {e:#}");
                }
            }
            let phase = jittered(timeout, errs);
            debug!("{name}: sleep for {phase:?}");
            sleep(phase).await;
        }
    }
}

struct Config {
    timeout: u64,
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
    ch.event_loop(&client, cfg.timeout).await;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .build()?;

    let cfg = Config {
        timeout: 10,
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
