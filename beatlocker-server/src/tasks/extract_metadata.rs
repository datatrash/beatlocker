use crate::AppResult;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use lewton::inside_ogg::OggStreamReader;
use ogg_metadata::{read_format, OggFormat};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{Read, Seek, SeekFrom};
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
}

pub fn extract_metadata(
    filename: Option<&OsStr>,
    mut reader: impl Clone + Read + Seek,
) -> AppResult<Option<SongMetadata>> {
    match read_format(reader.clone()) {
        Ok(formats) => {
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

                        // Try to extract the artist and title from the filename as well,
                        // for use when tags are missing
                        let filename_metadata = match filename {
                            Some(filename) => {
                                let filename = filename.to_string_lossy();

                                match filename.split_once('-') {
                                    Some((artist, title)) => (
                                        Some(title.trim().to_string()),
                                        Some(artist.trim().to_string()),
                                    ),
                                    None => (None, None),
                                }
                            }
                            None => (None, None),
                        };

                        let date = header
                            .remove("date")
                            .and_then(|s| {
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

                        Ok(Some(SongMetadata {
                            title: header
                                .remove("title")
                                .or(filename_metadata.0)
                                .unwrap_or_else(|| "Unknown".to_string()),
                            artist: header
                                .remove("artist")
                                .or(filename_metadata.1)
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
                        }))
                    }
                    _ => Ok(None),
                }
            } else {
                Ok(None)
            }
        }
        Err(e) => {
            warn!(?filename, ?e, "Could not parse metadata from file");
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn can_extract() {
        let bytes = include_bytes!("../../tests/data/Richard Bona/Richard Bona - Ba Senge.ogg");
        let metadata = extract_metadata(None, Cursor::new(bytes)).unwrap().unwrap();
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
    }
}
