use serde;
use serde::Deserialize;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .build()?;

    let res: PollRes = client.get(
        "https://api.chzzk.naver.com/polling/v2/channels/c847a58a1599988f6154446c75366523/live-status"
    ).send().await?.json().await?;
    println!("{res:?}");

    Ok(())
}
