use super::{SubtitleOption, SubtitleQuery, cache_subtitle};
use reqwest::Client;
use serde::Deserialize;
use std::{
    io::{Cursor, Read},
    path::PathBuf,
    time::Duration,
};
use zip::ZipArchive;

const API_URL: &str = "https://api.subdl.com/api/v1/subtitles";
const DOWNLOAD_BASE_URL: &str = "https://dl.subdl.com";
const API_KEY_ENV: &str = "SUBDL_API_KEY";
const MAX_SUBTITLE_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum SubDlError {
    #[error("SubDL request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("SubDL archive could not be read: {0}")]
    Archive(#[from] zip::result::ZipError),
    #[error("SubDL archive did not contain a supported subtitle file")]
    MissingSubtitle,
    #[error("SubDL API rejected the request: {0}")]
    Api(String),
    #[error("could not cache subtitle: {0}")]
    Cache(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct SubDlClient {
    client: Client,
    api_key: String,
}

impl SubDlClient {
    pub fn from_config() -> Result<Option<Self>, SubDlError> {
        let Some(api_key) = load_api_key() else {
            return Ok(None);
        };
        Ok(Some(Self {
            client: Client::builder().timeout(Duration::from_secs(15)).build()?,
            api_key,
        }))
    }

    pub async fn find_turkish(
        &self,
        query: &SubtitleQuery,
    ) -> Result<Option<SubtitleOption>, SubDlError> {
        let media_type = if query.season.is_some() {
            "tv"
        } else {
            "movie"
        };
        let response = self
            .client
            .get(API_URL)
            .query(&[
                ("api_key", self.api_key.clone()),
                ("film_name", query.title.clone()),
                ("type", media_type.to_string()),
                ("languages", "TR".to_string()),
                ("subs_per_page", "10".to_string()),
                ("unpack", "1".to_string()),
                ("client", "moviebox-tui".to_string()),
            ])
            .query(&query.year.map(|value| [("year", value.to_string())]))
            .query(
                &query
                    .season
                    .map(|value| [("season_number", value.to_string())]),
            )
            .query(
                &query
                    .episode
                    .map(|value| [("episode_number", value.to_string())]),
            )
            .send()
            .await?
            .error_for_status()?
            .json::<SubDlResponse>()
            .await?;

        if !response.status {
            return Err(SubDlError::Api(
                response
                    .error
                    .unwrap_or_else(|| "unknown API error".to_string()),
            ));
        }

        let Some(candidate) = first_candidate(&response, query) else {
            return Ok(None);
        };
        let url = absolute_download_url(&candidate.url);
        let contents = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        let (extension, contents) = if contents.starts_with(b"PK\x03\x04") {
            extract_subtitle(&contents)?
        } else {
            (candidate.extension, contents.to_vec())
        };

        Ok(Some(cache_subtitle(
            query,
            &extension,
            &contents,
            "Türkçe (SubDL)",
        )?))
    }
}

#[derive(Debug, Deserialize)]
struct SubDlResponse {
    #[serde(default)]
    status: bool,
    #[serde(default, alias = "message")]
    error: Option<String>,
    #[serde(default)]
    subtitles: Vec<Subtitle>,
}

#[derive(Debug, Deserialize)]
struct Subtitle {
    #[serde(default)]
    url: String,
    #[serde(default)]
    unpack_files: Vec<UnpackedFile>,
}

#[derive(Debug, Deserialize)]
struct UnpackedFile {
    #[serde(default)]
    url: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    season: Option<serde_json::Value>,
    #[serde(default)]
    episode: Option<serde_json::Value>,
}

struct Candidate {
    url: String,
    extension: String,
}

fn first_candidate(response: &SubDlResponse, query: &SubtitleQuery) -> Option<Candidate> {
    for subtitle in &response.subtitles {
        if let Some(file) = subtitle.unpack_files.iter().find(|file| {
            !file.url.is_empty()
                && is_supported(&file_extension(&file.name, &file.format))
                && matches_number(file.season.as_ref(), query.season)
                && matches_number(file.episode.as_ref(), query.episode)
        }) {
            return Some(Candidate {
                url: file.url.clone(),
                extension: file_extension(&file.name, &file.format),
            });
        }
        if !subtitle.url.is_empty() {
            return Some(Candidate {
                url: subtitle.url.clone(),
                extension: "srt".to_string(),
            });
        }
    }
    None
}

fn matches_number(value: Option<&serde_json::Value>, expected: Option<usize>) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    let Some(value) = value else {
        return true;
    };
    value.as_u64() == Some(expected as u64)
        || value.as_str().and_then(|value| value.parse::<usize>().ok()) == Some(expected)
}

fn extract_subtitle(archive: &[u8]) -> Result<(String, Vec<u8>), SubDlError> {
    let mut archive = ZipArchive::new(Cursor::new(archive))?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let extension = file_extension(file.name(), "");
        if file.is_dir() || !is_supported(&extension) || file.size() > MAX_SUBTITLE_BYTES {
            continue;
        }
        let mut contents = Vec::with_capacity(file.size() as usize);
        file.by_ref()
            .take(MAX_SUBTITLE_BYTES + 1)
            .read_to_end(&mut contents)?;
        if contents.len() as u64 <= MAX_SUBTITLE_BYTES {
            return Ok((extension, contents));
        }
    }
    Err(SubDlError::MissingSubtitle)
}

fn absolute_download_url(url: &str) -> String {
    if url.starts_with("https://") || url.starts_with("http://") {
        url.to_string()
    } else {
        format!("{DOWNLOAD_BASE_URL}/{}", url.trim_start_matches('/'))
    }
}

fn file_extension(name: &str, format: &str) -> String {
    PathBuf::from(name)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(format)
        .to_ascii_lowercase()
}

fn is_supported(extension: &str) -> bool {
    matches!(extension, "srt" | "vtt" | "ass" | "ssa")
}

fn load_api_key() -> Option<String> {
    if let Some(value) = std::env::var(API_KEY_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Some(value.trim().to_string());
    }
    let config_root = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|path| path.join(".config")))?;
    std::fs::read_to_string(config_root.join("moviebox-tui/subdl_api_key"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_an_unpacked_subtitle_file() {
        let response: SubDlResponse = serde_json::from_value(serde_json::json!({
            "status": true,
            "subtitles": [{
                "url": "/subtitle/123.zip",
                "unpack_files": [{
                    "name": "Movie.2026.tr.srt",
                    "format": "srt",
                    "url": "/subtitle/123/456"
                }]
            }]
        }))
        .unwrap();

        let candidate = first_candidate(
            &response,
            &SubtitleQuery {
                title: "Movie".to_string(),
                year: Some(2026),
                season: None,
                episode: None,
            },
        )
        .unwrap();
        assert_eq!(candidate.url, "/subtitle/123/456");
        assert_eq!(candidate.extension, "srt");
    }

    #[test]
    fn falls_back_to_archive_url() {
        let response: SubDlResponse = serde_json::from_value(serde_json::json!({
            "status": true,
            "subtitles": [{ "url": "/subtitle/123.zip" }]
        }))
        .unwrap();

        let candidate = first_candidate(
            &response,
            &SubtitleQuery {
                title: "Movie".to_string(),
                year: None,
                season: None,
                episode: None,
            },
        )
        .unwrap();
        assert_eq!(
            absolute_download_url(&candidate.url),
            "https://dl.subdl.com/subtitle/123.zip"
        );
    }

    #[test]
    fn selects_the_requested_episode_from_a_season_pack() {
        let response: SubDlResponse = serde_json::from_value(serde_json::json!({
            "status": true,
            "subtitles": [{
                "unpack_files": [
                    { "name": "S01E01.srt", "url": "/one", "season": 1, "episode": 1 },
                    { "name": "S01E02.srt", "url": "/two", "season": "1", "episode": "2" }
                ]
            }]
        }))
        .unwrap();
        let query = SubtitleQuery {
            title: "Show".to_string(),
            year: None,
            season: Some(1),
            episode: Some(2),
        };

        assert_eq!(first_candidate(&response, &query).unwrap().url, "/two");
    }
}
