use chrono::{DateTime, Utc};

pub struct Release {
    pub album: Option<(ProviderUri, String)>,
    pub album_artist: Option<(ProviderUri, String)>,
    pub artist: Option<(ProviderUri, String)>,
    pub song: (ProviderUri, String),
    pub genre: Option<String>,
    pub release_date: Option<DateTime<Utc>>,
}

pub struct ProviderUri(String);

impl ToString for ProviderUri {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl ProviderUri {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_provider(provider: &str, uri: &str) -> Self {
        Self(format!("{provider}:{uri}"))
    }
}
