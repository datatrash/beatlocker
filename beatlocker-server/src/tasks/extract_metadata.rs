use crate::{AppError, AppResult};
use anyhow::anyhow;
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, Utc};
use heck::ToTitleCase;
use lewton::inside_ogg::OggStreamReader;
use std::ffi::OsStr;
use std::path::PathBuf;
use symphonia::core::codecs::{CODEC_TYPE_FLAC, CODEC_TYPE_MP3, CODEC_TYPE_VORBIS};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::{MetadataOptions, StandardTagKey};
use symphonia::core::probe::Hint;
use symphonia_metadata::id3v1;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SongMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub bit_rate: Option<u32>,
    pub duration: Option<Duration>,
    pub genre: Option<String>,
    pub content_type: Option<String>,
    pub suffix: Option<String>,
}

impl SongMetadata {
    pub fn artist(&self) -> &str {
        self.artist.as_deref().unwrap()
    }

    pub fn is_valid(&self) -> bool {
        self.title.is_some() && self.artist.is_some()
    }
}

pub fn extract_metadata(
    filename: &OsStr,
    reader: impl Fn() -> Box<dyn MediaSource>,
) -> AppResult<Option<SongMetadata>> {
    let metadata: Option<SongMetadata> = {
        let mss = MediaSourceStream::new(reader(), Default::default());

        let suffix = PathBuf::from(filename)
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase());

        // Create a probe hint using the file's extension. [Optional]
        let mut hint = Hint::new();
        suffix.as_ref().map(|s| hint.with_extension(s));

        // Use the default options for metadata and format readers.
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        // Probe the media source.
        let mut probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .map_err(|_| anyhow!("Unsupported format"))?;

        let mut format = probed.format;
        let track = format
            .default_track()
            .ok_or_else(|| AppError(anyhow!("No supported audio tracks")))?;
        let codec_params = track.codec_params.clone();
        let content_type = match &codec_params.codec {
            _ if codec_params.codec == CODEC_TYPE_VORBIS => Some("audio/ogg".to_string()),
            _ if codec_params.codec == CODEC_TYPE_MP3 => Some("audio/mp3".to_string()),
            _ if codec_params.codec == CODEC_TYPE_FLAC => Some("audio/flac".to_string()),
            _ => None,
        };

        let bit_rate = match codec_params.bits_per_coded_sample {
            Some(val) => Some(val),
            None => match &codec_params.codec {
                _ if codec_params.codec == CODEC_TYPE_VORBIS => OggStreamReader::new(reader())
                    .ok()
                    .map(|h| (h.ident_hdr.bitrate_nominal / 1000) as u32),
                _ => None,
            },
        };

        let metadata = SongMetadata {
            bit_rate,
            duration: codec_params.time_base.and_then(|tb| {
                codec_params
                    .n_frames
                    .map(|nf| Duration::seconds(tb.calc_time(nf).seconds as i64))
            }),
            content_type,
            suffix,
            ..Default::default()
        };

        let probed_metadata = probed
            .metadata
            .get()
            .and_then(|mut m| m.skip_to_latest().cloned());
        let format_metadata = format.metadata().skip_to_latest().cloned();

        if let Some(rev) = format_metadata.or(probed_metadata) {
            let get_value = |wanted_key: StandardTagKey| {
                rev.tags()
                    .iter()
                    .find(|tag| tag.std_key.map(|key| key == wanted_key).unwrap_or_default())
                    .map(|tag| tag.value.to_string())
            };
            Some(SongMetadata {
                title: get_value(StandardTagKey::TrackTitle),
                artist: get_value(StandardTagKey::Artist),
                album: get_value(StandardTagKey::Album),
                album_artist: get_value(StandardTagKey::AlbumArtist)
                    .or_else(|| get_value(StandardTagKey::Artist)),
                date: get_value(StandardTagKey::Date)
                    .or_else(|| get_value(StandardTagKey::ReleaseDate))
                    .and_then(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                            .or_else(|| {
                                NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                                    .ok()
                                    .map(|d| d.and_time(NaiveTime::default()))
                                    .map(|dt| dt.and_local_timezone(Utc).unwrap())
                            })
                            .or_else(|| {
                                s.parse::<u32>()
                                    .ok()
                                    .and_then(|year| DateTime::default().with_year(year as i32))
                            })
                    }),
                track_number: get_value(StandardTagKey::TrackNumber).and_then(|t| t.parse().ok()),
                disc_number: get_value(StandardTagKey::DiscNumber).and_then(|t| t.parse().ok()),
                genre: get_value(StandardTagKey::Genre).map(|genre_id| {
                    // id is potentially in format "(181)"
                    match genre_id[1..genre_id.len() - 1].parse::<u8>().ok() {
                        Some(id) => match id3v1::util::genre_name(id).map(|g| g.to_string()) {
                            Some(genre) => genre,
                            None => genre_id,
                        },
                        None => genre_id,
                    }
                }),
                ..metadata
            })
        } else {
            Some(metadata)
        }
    };

    // Try to extract the artist and title from the filename as well,
    // for use when tags are missing
    let (title, artist) = {
        let without_extension = PathBuf::from(filename).with_extension("");
        let filename = without_extension.to_string_lossy();

        match filename.split_once('-') {
            Some((artist, title)) => (
                Some(title.trim().to_string().to_title_case()),
                Some(artist.trim().to_string().to_title_case()),
            ),
            None => (None, None),
        }
    };

    match metadata {
        Some(mut metadata) => {
            if let Some(title) = title {
                if metadata.title.is_none() {
                    metadata.title = Some(title);
                }
            }
            if let Some(artist) = artist {
                if metadata.artist.is_none() {
                    metadata.artist = Some(artist.clone());
                    metadata.album_artist = Some(artist);
                }
            }

            Ok(Some(metadata))
        }
        None => {
            // We didn't extract any metadata, so let's try to use title/artist
            match (title, artist) {
                (Some(title), Some(artist)) => Ok(Some(SongMetadata {
                    title: Some(title),
                    artist: Some(artist.clone()),
                    album_artist: Some(artist),
                    ..Default::default()
                })),
                _ => Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn can_extract_ogg() {
        let bytes = include_bytes!("../../tests/data/Richard Bona/Richard Bona - Ba Senge.ogg");
        let metadata = extract_metadata(OsStr::new("Richard Bona - Ba Senge.ogg"), || {
            Box::new(Cursor::new(bytes))
        })
        .unwrap()
        .unwrap();

        assert!(metadata.is_valid());
        assert_eq!(metadata.title, Some("Ba Senge".to_string()));
        assert_eq!(metadata.album, Some("Tiki".to_string()));
        assert_eq!(metadata.artist, Some("Richard Bona".to_string()));
        assert_eq!(metadata.album_artist, Some("Richard Bona".to_string()));
        assert_eq!(
            metadata.date.map(|d| d.to_rfc3339()),
            Some("2021-12-02T00:00:00+00:00".to_string())
        );
        assert_eq!(metadata.track_number, Some(1));
        assert_eq!(metadata.disc_number, Some(1));
        assert_eq!(metadata.content_type, Some("audio/ogg".to_string()));
        assert_eq!(metadata.suffix, Some("ogg".to_string()));
        assert_eq!(metadata.bit_rate, Some(160));
        assert_eq!(metadata.duration, Some(Duration::seconds(6)));
    }

    #[test]
    fn can_extract_mp3() {
        let bytes =
            include_bytes!("../../tests/data/Richard Bona/Richard Bona - Akwa Samba Yaya.mp3");
        let metadata = extract_metadata(OsStr::new("Richard Bona - Akwa Samba Yaya.mp3"), || {
            Box::new(Cursor::new(bytes))
        })
        .unwrap()
        .unwrap();

        assert!(metadata.is_valid());
        assert_eq!(metadata.title, Some("Akwa Samba Yaya".to_string()));
        assert_eq!(metadata.album, Some("Tiki".to_string()));
        assert_eq!(metadata.artist, Some("Richard Bona".to_string()));
        assert_eq!(metadata.album_artist, Some("Richard Bona".to_string()));
        assert_eq!(metadata.genre, Some("World Music".to_string()));
        assert_eq!(
            metadata.date.map(|d| d.to_rfc3339()),
            Some("2021-01-01T00:00:00+00:00".to_string())
        );
        assert_eq!(metadata.track_number, Some(2));
        assert_eq!(metadata.disc_number, None);
        assert_eq!(metadata.content_type, Some("audio/mp3".to_string()));
        assert_eq!(metadata.suffix, Some("mp3".to_string()));
        assert_eq!(metadata.bit_rate, None);
        assert_eq!(metadata.duration, Some(Duration::seconds(27)));
    }

    #[test]
    fn can_extract_flac() {
        let bytes = include_bytes!(
            "../../tests/data/Motorway OST/MotorwayNested/Alex Gopher - Radar Unit.flac"
        );
        let metadata = extract_metadata(OsStr::new("Alex Gopher - Radar Unit.flac"), || {
            Box::new(Cursor::new(bytes))
        })
        .unwrap()
        .unwrap();

        assert!(metadata.is_valid());
        assert_eq!(metadata.title, Some("Radar Unit".to_string()));
        assert_eq!(
            metadata.album,
            Some("Motorway (Original Motion Picture Soundtrack)".to_string())
        );
        assert_eq!(metadata.artist, Some("Alex Gopher".to_string()));
        assert_eq!(metadata.album_artist, Some("Alex Gopher".to_string()));
        assert_eq!(metadata.genre, None);
        assert_eq!(metadata.date, None);
        assert_eq!(metadata.track_number, None);
        assert_eq!(metadata.disc_number, None);
        assert_eq!(metadata.content_type, Some("audio/flac".to_string()));
        assert_eq!(metadata.suffix, Some("flac".to_string()));
        assert_eq!(metadata.bit_rate, None);
        assert_eq!(metadata.duration, Some(Duration::seconds(95)));
    }

    #[test]
    fn can_extract_unknown_metadata() {
        let bytes = include_bytes!("../../tests/data/Unknown/Unknown Artist - Unknown Song.ogg");
        let metadata =
            extract_metadata(OsStr::new("Foo - Bar.ogg"), || Box::new(Cursor::new(bytes)))
                .unwrap()
                .unwrap();
        assert!(metadata.is_valid());
        assert_eq!(metadata.title, Some("Bar".to_string()));
        assert_eq!(metadata.album, None);
        assert_eq!(metadata.artist, Some("Foo".to_string()));
        assert_eq!(metadata.album_artist, Some("Foo".to_string()));
        assert_eq!(metadata.date, None);
        assert_eq!(metadata.track_number, None);
        assert_eq!(metadata.disc_number, None);
        assert_eq!(metadata.content_type, Some("audio/ogg".to_string()));
        assert_eq!(metadata.suffix, Some("ogg".to_string()));
    }
}
