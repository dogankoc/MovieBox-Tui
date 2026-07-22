use md5::{Digest, Md5};
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use serde_json::{Value, json};
use std::{collections::HashSet, path::PathBuf, time::Duration};

pub mod subdl;

const API_BASE_URL: &str = "https://api.opensubtitles.com/api/v1";
const API_KEY_ENV: &str = "OPENSUBTITLES_API_KEY";
const USER_AGENT_ENV: &str = "OPENSUBTITLES_USER_AGENT";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleOption {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleQuery {
    pub title: String,
    pub year: Option<u16>,
    pub season: Option<usize>,
    pub episode: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenSubtitlesError {
    #[error("invalid OpenSubtitles header value")]
    InvalidHeader,
    #[error("OpenSubtitles request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("OpenSubtitles response did not contain a download link")]
    MissingDownloadLink,
    #[error("could not cache subtitle: {0}")]
    Cache(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct OpenSubtitlesClient {
    client: reqwest::Client,
    api_key: String,
    user_agent: String,
}

impl OpenSubtitlesClient {
    pub fn from_config() -> Result<Option<Self>, OpenSubtitlesError> {
        let Some(api_key) = load_api_key() else {
            return Ok(None);
        };

        let user_agent = std::env::var(USER_AGENT_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("MovieBox-Tui v{}", env!("CARGO_PKG_VERSION")));

        Ok(Some(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()?,
            api_key,
            user_agent,
        }))
    }

    pub async fn find_turkish(
        &self,
        query: &SubtitleQuery,
    ) -> Result<Option<SubtitleOption>, OpenSubtitlesError> {
        let response = self
            .client
            .get(format!("{API_BASE_URL}/subtitles"))
            .headers(self.headers(false)?)
            .query(&[
                ("languages", "tr".to_string()),
                ("query", query.title.clone()),
                ("order_by", "download_count".to_string()),
                ("order_direction", "desc".to_string()),
            ])
            .query(&query.year.map(|year| [("year", year.to_string())]))
            .query(
                &query
                    .season
                    .map(|season| [("season_number", season.to_string())]),
            )
            .query(
                &query
                    .episode
                    .map(|episode| [("episode_number", episode.to_string())]),
            )
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        let Some(file_id) = first_file_id(&response) else {
            return Ok(None);
        };

        let response = self
            .client
            .post(format!("{API_BASE_URL}/download"))
            .headers(self.headers(true)?)
            .json(&json!({ "file_id": file_id, "sub_format": "srt" }))
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        let url = download_link(&response).ok_or(OpenSubtitlesError::MissingDownloadLink)?;
        let contents = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        Ok(Some(cache_subtitle(
            query,
            "srt",
            &contents,
            "Türkçe (OpenSubtitles)",
        )?))
    }

    fn headers(&self, json_body: bool) -> Result<HeaderMap, OpenSubtitlesError> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            "Api-Key",
            HeaderValue::from_str(&self.api_key).map_err(|_| OpenSubtitlesError::InvalidHeader)?,
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&self.user_agent)
                .map_err(|_| OpenSubtitlesError::InvalidHeader)?,
        );
        if json_body {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }
        Ok(headers)
    }
}

pub fn moviebox_subtitles(payload: &Value) -> Vec<SubtitleOption> {
    let mut seen_urls = HashSet::new();
    let mut subtitles = payload
        .get("extCaptions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|caption| {
            let url = caption.get("url").and_then(Value::as_str)?.trim();
            if url.is_empty() || !seen_urls.insert(url.to_string()) {
                return None;
            }
            let name = caption
                .get("lanName")
                .and_then(Value::as_str)
                .unwrap_or("Unknown")
                .to_string();
            Some(SubtitleOption {
                name,
                url: url.to_string(),
            })
        })
        .collect::<Vec<_>>();

    subtitles.sort_by_key(|subtitle| !is_turkish_label(&subtitle.name));
    subtitles
}

pub fn has_turkish(subtitles: &[SubtitleOption]) -> bool {
    subtitles
        .iter()
        .any(|subtitle| is_turkish_label(&subtitle.name))
}

pub fn options_with_none(subtitles: Vec<SubtitleOption>) -> Vec<(String, String)> {
    let has_preferred = has_turkish(&subtitles);
    let mut options = subtitles
        .into_iter()
        .map(|subtitle| (subtitle.name, subtitle.url))
        .collect::<Vec<_>>();
    let none = ("None".to_string(), String::new());
    if has_preferred {
        options.push(none);
    } else {
        options.insert(0, none);
    }
    options
}

pub fn is_turkish_label(label: &str) -> bool {
    let normalized = label.trim().to_lowercase();
    matches!(normalized.as_str(), "tr" | "tur")
        || normalized.contains("turkish")
        || normalized.contains("türkçe")
}

pub fn cached_turkish(query: &SubtitleQuery) -> Option<SubtitleOption> {
    let directory = subtitle_cache_dir()?;
    ["srt", "vtt", "ass"]
        .into_iter()
        .map(|extension| directory.join(format!("{}.{}", cache_key(query), extension)))
        .find(|path| path.is_file())
        .map(|path| SubtitleOption {
            name: "Türkçe (önbellek)".to_string(),
            url: path.to_string_lossy().into_owned(),
        })
}

pub(crate) fn cache_subtitle(
    query: &SubtitleQuery,
    extension: &str,
    contents: &[u8],
    name: &str,
) -> Result<SubtitleOption, std::io::Error> {
    let directory = subtitle_cache_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cache directory is unavailable",
        )
    })?;
    std::fs::create_dir_all(&directory)?;
    let extension = match extension.to_ascii_lowercase().as_str() {
        "vtt" => "vtt",
        "ass" | "ssa" => "ass",
        _ => "srt",
    };
    let path = directory.join(format!("{}.{}", cache_key(query), extension));
    std::fs::write(&path, contents)?;
    Ok(SubtitleOption {
        name: name.to_string(),
        url: path.to_string_lossy().into_owned(),
    })
}

fn subtitle_cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|path| path.join("moviebox-tui/subtitles"))
}

fn cache_key(query: &SubtitleQuery) -> String {
    let identity = format!(
        "{}|{}|{}|{}",
        query.title.trim().to_lowercase(),
        query
            .year
            .map_or_else(String::new, |value| value.to_string()),
        query
            .season
            .map_or_else(String::new, |value| value.to_string()),
        query
            .episode
            .map_or_else(String::new, |value| value.to_string())
    );
    let mut hasher = Md5::new();
    hasher.update(identity.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn load_api_key() -> Option<String> {
    if let Some(value) = std::env::var(API_KEY_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Some(value.trim().to_string());
    }

    let path = config_key_path()?;
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn config_key_path() -> Option<PathBuf> {
    let config_root = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    config_key_path_from(config_root, dirs::home_dir())
}

fn config_key_path_from(config_root: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    let config_root = config_root.or_else(|| home.map(|path| path.join(".config")))?;
    Some(config_root.join("moviebox-tui/opensubtitles_api_key"))
}

fn first_file_id(payload: &Value) -> Option<i64> {
    payload
        .get("data")?
        .as_array()?
        .iter()
        .filter(|item| {
            item.get("attributes")
                .and_then(|attributes| attributes.get("language"))
                .and_then(Value::as_str)
                .is_some_and(|language| language.eq_ignore_ascii_case("tr"))
        })
        .find_map(|item| {
            item.get("attributes")?
                .get("files")?
                .as_array()?
                .first()?
                .get("file_id")?
                .as_i64()
        })
}

fn download_link(payload: &Value) -> Option<&str> {
    payload.get("link").and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_turkish_labels() {
        for label in ["Turkish", "Türkçe", "TR", "tur"] {
            assert!(is_turkish_label(label), "{label}");
        }
        assert!(!is_turkish_label("Turkmen"));
        assert!(!is_turkish_label("English"));
    }

    #[test]
    fn prioritizes_turkish_and_selects_it_by_default() {
        let payload = json!({
            "extCaptions": [
                { "lanName": "English", "url": "https://example.com/en.srt" },
                { "lanName": "Türkçe", "url": "https://example.com/tr.srt" }
            ]
        });

        let options = options_with_none(moviebox_subtitles(&payload));
        assert_eq!(options[0].0, "Türkçe");
        assert_eq!(options.last().unwrap().0, "None");
    }

    #[test]
    fn keeps_none_selected_when_turkish_is_unavailable() {
        let payload = json!({
            "extCaptions": [
                { "lanName": "English", "url": "https://example.com/en.srt" }
            ]
        });

        let options = options_with_none(moviebox_subtitles(&payload));
        assert_eq!(options[0].0, "None");
    }

    #[test]
    fn extracts_file_id_and_download_link() {
        let search = json!({
            "data": [{
                "attributes": {
                    "language": "tr",
                    "files": [{ "file_id": 12345 }]
                }
            }]
        });
        let download = json!({ "link": "https://example.com/subtitle.srt" });

        assert_eq!(first_file_id(&search), Some(12345));
        assert_eq!(
            download_link(&download),
            Some("https://example.com/subtitle.srt")
        );
    }

    #[test]
    fn uses_the_same_config_path_as_the_installer() {
        let path = config_key_path_from(None, Some(PathBuf::from("/Users/example")));
        assert_eq!(
            path,
            Some(PathBuf::from(
                "/Users/example/.config/moviebox-tui/opensubtitles_api_key"
            ))
        );
    }

    #[test]
    fn cache_key_is_stable_and_episode_specific() {
        let movie = SubtitleQuery {
            title: "  Backrooms ".to_string(),
            year: Some(2026),
            season: None,
            episode: None,
        };
        let same_movie = SubtitleQuery {
            title: "backrooms".to_string(),
            year: Some(2026),
            season: None,
            episode: None,
        };
        let episode = SubtitleQuery {
            season: Some(1),
            episode: Some(1),
            ..same_movie.clone()
        };

        assert_eq!(cache_key(&movie), cache_key(&same_movie));
        assert_ne!(cache_key(&movie), cache_key(&episode));
    }
}
