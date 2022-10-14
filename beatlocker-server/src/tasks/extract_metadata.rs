use crate::AppResult;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use lewton::inside_ogg::OggStreamReader;
use ogg_metadata::{read_format, OggFormat};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use tracing::warn;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SongMetadata {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub bit_rate: Option<u32>,
    pub duration: Option<chrono::Duration>,
    pub genre: Option<String>,
    pub content_type: Option<String>,
    pub suffix: Option<String>,
}

pub fn extract_metadata(
    filename: &OsStr,
    mut reader: impl Clone + Read + Seek,
) -> AppResult<Option<SongMetadata>> {
    let mut file_is_valid = false;

    let metadata = match read_format(reader.clone()) {
        Ok(formats) => {
            file_is_valid = true;
            if let Some(format) = formats.first() {
                match format {
                    OggFormat::Vorbis(vorbis) => {
                        reader.seek(SeekFrom::Start(0))?;
                        let duration_seconds = (vorbis.length_in_samples.unwrap_or_default()
                            as f32)
                            / vorbis.sample_rate as f32;
                        let headers = OggStreamReader::new(reader)?;
                        let bit_rate = headers.ident_hdr.bitrate_nominal / 1000;
                        let mut header = headers
                            .comment_hdr
                            .comment_list
                            .into_iter()
                            .collect::<HashMap<_, _>>();

                        let date = header.remove("date").and_then(|s| {
                            DateTime::parse_from_rfc3339(&s)
                                .ok()
                                .map(|dt| dt.with_timezone(&Utc))
                                .or_else(|| {
                                    let date = NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok();
                                    let date = date.map(|d| {
                                        d.and_time(NaiveTime::default()).and_local_timezone(Utc)
                                    });

                                    date.map(|d| d.unwrap())
                                })
                        });

                        let suffix = PathBuf::from(filename)
                            .extension()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_lowercase());
                        let content_type = match &suffix {
                            Some(ct) => match ct.as_str() {
                                "ogg" => Some("audio/ogg".to_string()),
                                _ => None,
                            },
                            _ => None,
                        };

                        Some(SongMetadata {
                            title: header
                                .remove("title")
                                .unwrap_or_else(|| "Unknown".to_string()),
                            artist: header
                                .remove("artist")
                                .unwrap_or_else(|| "Unknown artist".to_string()),
                            album: header.remove("album"),
                            album_artist: header.remove("albumartist"),
                            date,
                            track_number: header
                                .remove("tracknumber")
                                .map(|s| s.parse())
                                .transpose()?,
                            disc_number: header
                                .remove("discnumber")
                                .map(|s| s.parse())
                                .transpose()?,
                            bit_rate: Some(bit_rate as u32),
                            duration: Some(chrono::Duration::seconds(
                                duration_seconds.round() as i64
                            )),
                            genre: header.remove("genre"),
                            content_type,
                            suffix,
                        })
                    }
                    _ => None,
                }
            } else {
                None
            }
        }
        Err(e) => {
            warn!(?filename, ?e, "Could not parse metadata from file");
            None
        }
    };

    if !file_is_valid {
        return Ok(None);
    }

    // Try to extract the artist and title from the filename as well,
    // for use when tags are missing
    let (title, artist) = {
        let without_extension = PathBuf::from(filename).with_extension("");
        let filename = without_extension.to_string_lossy();

        match filename.split_once('-') {
            Some((artist, title)) => (
                Some(title.trim().to_string()),
                Some(artist.trim().to_string()),
            ),
            None => (None, None),
        }
    };

    match metadata {
        Some(mut metadata) => {
            if let Some(title) = title {
                if metadata.title == "Unknown" {
                    metadata.title = title;
                }
            }
            if let Some(artist) = artist {
                if metadata.artist == "Unknown artist" {
                    metadata.artist = artist.clone();
                    metadata.album_artist = Some(artist);
                }
            }

            Ok(Some(metadata))
        }
        None => {
            // We didn't extract any metadata, so let's try to use title/artist
            match (title, artist) {
                (Some(title), Some(artist)) => Ok(Some(SongMetadata {
                    title,
                    artist: artist.clone(),
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
        let metadata = extract_metadata(
            OsStr::new("Richard Bona - Ba Senge.ogg"),
            Cursor::new(bytes),
        )
        .unwrap()
        .unwrap();
        assert_eq!(metadata.title, "Ba Senge");
        assert_eq!(metadata.album, Some("Tiki".to_string()));
        assert_eq!(metadata.artist, "Richard Bona".to_string());
        assert_eq!(metadata.album_artist, Some("Richard Bona".to_string()));
        assert_eq!(
            metadata.date.map(|d| d.to_rfc3339()),
            Some("2021-12-02T00:00:00+00:00".to_string())
        );
        assert_eq!(metadata.track_number, Some(1));
        assert_eq!(metadata.disc_number, Some(1));
        assert_eq!(metadata.content_type, Some("audio/ogg".to_string()));
        assert_eq!(metadata.suffix, Some("ogg".to_string()));
    }

    #[test]
    fn can_extract_unknown_metadata() {
        let bytes = include_bytes!("../../tests/data/Unknown/Unknown Artist - Unknown Song.ogg");
        let metadata = extract_metadata(OsStr::new("Foo - Bar.ogg"), Cursor::new(bytes))
            .unwrap()
            .unwrap();
        assert_eq!(metadata.title, "Bar");
        assert_eq!(metadata.album, None);
        assert_eq!(metadata.artist, "Foo".to_string());
        assert_eq!(metadata.album_artist, Some("Foo".to_string()));
        assert_eq!(metadata.date, None);
        assert_eq!(metadata.track_number, None);
        assert_eq!(metadata.disc_number, None);
        assert_eq!(metadata.content_type, Some("audio/ogg".to_string()));
        assert_eq!(metadata.suffix, Some("ogg".to_string()));
    }
}
