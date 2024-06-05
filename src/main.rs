use anyhow::anyhow;
use reqwest::Response;
use serde::{ Deserialize, Serialize };
use webhook::client::WebhookClient;
use webhook::models::AllowedMention;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use tracing::Level;
use tracing_appender::non_blocking::NonBlocking;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::SubscriberBuilder;

use rusqlite::Connection;

extern crate tokio;
mod data;
use crate::data::Feed;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub playlist: String,
    pub title: String,
    pub author: String,
    pub timestamp: String,
    pub hooked: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub playlist: Vec<Playlist>,
    pub log_level: Option<String>,
    pub author: User,
    pub bot: User,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub url: String,
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub webhooks: Vec<Webhook>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Webhook {
    pub destination: WebhookType,
    pub is_forum: bool,
    pub urls: Vec<String>,
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WebhookType {
    #[serde(rename = "discord")]
    Discord,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config: Config = match fs::read_to_string("data.hcl") {
        Ok(data) =>
            match serde_yaml::from_str(data.as_str()) {
                Ok(config) => config,
                Err(e) => {
                    println!("Error: {:?}", e);
                    return Err(anyhow!(e.to_string()));
                }
            }
        Err(e) => {
            println!("Error: {}", e.to_string());
            return Err(anyhow!(e.to_string()));
        }
    };

    let log_level = match config.log_level {
        Some(level) => level,
        None => "INFO".to_string(),
    };

    let level: tracing::Level = Level::from_str(log_level.as_str()).unwrap();
    let subscriber: SubscriberBuilder = tracing_subscriber::fmt();
    let non_blocking: NonBlocking;
    let _guard: WorkerGuard;
    (non_blocking, _guard) = tracing_appender::non_blocking(std::io::stdout());

    subscriber
        .with_writer(non_blocking)
        .with_max_level(level)
        .with_level(true)
        .with_line_number(level == tracing::Level::TRACE)
        .with_file(level == tracing::Level::TRACE)
        .compact()
        .init();

    let client = match
        reqwest::Client
            ::builder()
            .brotli(true)
            .gzip(true)
            .https_only(true)
            .connection_verbose(true)
            .use_rustls_tls()
            .build()
    {
        Ok(client) => client,
        Err(e) => {
            return Err(anyhow!(e.to_string()));
        }
    };

    let connection = Connection::open(Path::new("videos.sqlite3"))?;
    connection.execute(
        "CREATE TABLE IF NOT EXISTS video (id VARCHAR(255) PRIMARY KEY, playlist VARCHAR(255), title VARCHAR(255), author VARCHAR(255), timestamp DATETIME UNIQUE, hooked BOOLEAN DEFAULT 0)",
        ()
    )?;

    // Iterate over all the playlists, then store a basic record in sqlite
    for playlist in &config.playlist {
        // Fetch the records
        let results = match
            client
                .get(
                    format!("https://www.youtube.com/feeds/videos.xml?playlist_id={}", playlist.id)
                )
                .send().await
                .expect("failed to get response")
                .text().await
        {
            Ok(response) =>
                match quick_xml::de::from_str::<Feed>(&response) {
                    Ok(data) => Some(data),
                    Err(e) => {
                        tracing::error!("{}", e.to_string());
                        None
                    }
                }
            Err(e) => {
                tracing::error!("{}", e.to_string());
                None
            }
        };

        // Insert the records into the database
        match results {
            Some(data) => {
                for entry in data.entry {
                    let r = connection.execute(
                        "INSERT OR IGNORE INTO video (id, title, playlist, timestamp, hooked) VALUES (?1, ?2, ?3, ?4, 0);",
                        (
                            &entry.id.replace("yt:video:", ""),
                            &entry.title,
                            &playlist.id,
                            &entry.published,
                        )
                    );
                }
            }
            None => {}
        }
    }

    // Once we have all the videos, we need to run the webhooks for them
    for playlist in &config.playlist {
        let mut stmt = connection
            .prepare(
                &format!(
                    "SELECT * FROM video WHERE hooked = 0 AND playlist = '{}' ORDER BY timestamp ASC",
                    &playlist.id
                )
            )
            .unwrap();
        let videos = stmt
            .query_map([], |row| {
                Ok(Video {
                    id: row.get(0).unwrap(),
                    playlist: row.get(1).unwrap(),
                    title: row.get(2).unwrap(),
                    author: String::from(""),
                    timestamp: row.get(4).unwrap(),
                    hooked: row.get(5).unwrap(),
                })
            })
            .unwrap();

        for video in videos {
            if video.is_ok() {
                let v = video.unwrap();
                for webhook in &playlist.webhooks {
                    match webhook.destination {
                        WebhookType::Discord => {
                            for url in &webhook.urls {
                                let client: WebhookClient = WebhookClient::new(url);
                                match
                                    client.send(|message|
                                        message
                                            .username(&config.bot.name)
                                            .avatar_url(&config.bot.icon)
                                            .content(
                                                &format!(
                                                    "{} :: {}",
                                                    webhook.groups.join(" ").as_str(),
                                                    &format!(
                                                        "https://www.youtube.com/watch?v={}",
                                                        &v.id
                                                    )
                                                )
                                            )
                                            .thread_name(&v.title, webhook.is_forum)
                                            .allow_mentions(
                                                Some(
                                                    vec![
                                                        AllowedMention::UserMention,
                                                        AllowedMention::RoleMention
                                                    ]
                                                ),
                                                None,
                                                None,
                                                false
                                            )
                                            .embed(|embed|
                                                embed
                                                    .description(
                                                        &format!(
                                                            "### [{}]({})",
                                                            &v.title,
                                                            &format!(
                                                                "https://www.youtube.com/watch?v={}",
                                                                &v.id
                                                            )
                                                        )
                                                    )
                                                    .color(&"16711680")
                                                    .image(
                                                        &format!(
                                                            "https://img.youtube.com/vi/{}/maxresdefault.jpg",
                                                            &v.id
                                                        )
                                                    )
                                                    .video(
                                                        &format!(
                                                            "https://www.youtube.com/watch?v={}",
                                                            &v.id
                                                        )
                                                    )
                                                    .thumbnail(
                                                        &format!(
                                                            "https://www.iconfinder.com/icons/317714/download/png/256"
                                                        )
                                                    )
                                                    .author(
                                                        &config.author.name,
                                                        Some(config.author.clone().url),
                                                        Some(config.author.clone().icon)
                                                    )
                                            )
                                    ).await
                                {
                                    Ok(result) => {
                                        if result {
                                            let r = connection.execute(
                                                &format!(
                                                    "UPDATE video SET hooked = 1 WHERE id = '{}';",
                                                    &v.id
                                                ),
                                                ()
                                            );
                                        }
                                        _ = tokio::time::sleep(Duration::from_secs(1)).await;
                                    }
                                    Err(e) => {
                                        tracing::error!("{:?}", e);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    return Ok(());
}
