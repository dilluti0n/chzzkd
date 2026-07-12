use serde;
use serde::Deserialize;
use std::time::Duration;
use std::sync::Arc;
use std::fmt;
use rand::RngExt;

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

struct Config {
    timeout: u64,
    channel: Vec<Channel>,
}

fn jittered(timeout: u64, errs: u32) -> Duration {
    let backoff = 1u64 << errs.min(3); // 1x 2x 4x 8x
    let secs = (timeout * backoff) as f64 * rand::rng().random_range(0.9..1.1);

    Duration::from_secs_f64(secs)
}

async fn fetch(client: &reqwest::Client, ch: &Channel) -> Result<PollRes, reqwest::Error> {
    let id = &ch.id;
    let url = format!("https://api.chzzk.naver.com/polling/v2/channels/{id}/live-status");

    client.get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

async fn watch(cfg: Arc<Config>, idx: usize, client: reqwest::Client) {
    use tokio::time::sleep;

    let ch = &cfg.channel[idx];
    let name = ch.to_string();
    let phase = Duration::from_secs_f64(rand::rng().random_range(0.0..cfg.timeout as f64));

    sleep(phase).await;

    let mut errs = 0u32;
    loop {
        match fetch(&client, ch).await {
            Ok(res) => {
                eprintln!("{name}: {res:?}");
            }
            Err(e) => {
                errs = errs.saturating_add(1);
                eprintln!("{name}: fetch failed (x{errs}): {e:#}");
            }
        }
        let phase = jittered(cfg.timeout, errs);
        eprintln!("{name}: sleep for {phase:?}");
        sleep(phase).await;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
