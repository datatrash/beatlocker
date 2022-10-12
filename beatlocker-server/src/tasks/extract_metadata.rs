use crate::AppResult;
use lewton::inside_ogg::OggStreamReader;
use ogg_metadata::{read_format, OggFormat};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SongMetadata {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub date: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub bit_rate: Option<u32>,
    pub duration: Option<chrono::Duration>,
}

pub fn extract_metadata(mut reader: impl Clone + Read + Seek) -> AppResult<Option<SongMetadata>> {
    if let Some(format) = read_format(reader.clone())?.first() {
        match format {
            OggFormat::Vorbis(vorbis) => {
                reader.seek(SeekFrom::Start(0))?;
                let duration_seconds = (vorbis.length_in_samples.unwrap_or_default() as f32)
                    / vorbis.sample_rate as f32;
                let headers = OggStreamReader::new(reader)?;
                let bit_rate = headers.ident_hdr.bitrate_nominal / 1000;
                let mut header = headers
                    .comment_hdr
                    .comment_list
                    .into_iter()
                    .collect::<HashMap<_, _>>();

                Ok(Some(SongMetadata {
                    title: header.remove("title").unwrap(),
                    artist: header.remove("artist").unwrap(),
                    album: header.remove("album"),
                    album_artist: header.remove("albumartist"),
                    date: header.remove("date"),
                    track_number: header
                        .remove("tracknumber")
                        .map(|s| s.parse())
                        .transpose()?,
                    disc_number: header.remove("discnumber").map(|s| s.parse()).transpose()?,
                    bit_rate: Some(bit_rate as u32),
                    duration: Some(chrono::Duration::seconds(duration_seconds.round() as i64)),
                }))
            }
            _ => Ok(None),
        }
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::tasks::extract_metadata::extract_metadata;
    use std::io::Cursor;

    #[test]
    fn can_extract() {
        let bytes = include_bytes!("test-data/test.ogg");
        let metadata = extract_metadata(Cursor::new(bytes)).unwrap().unwrap();
        assert_eq!(metadata.title, "Title");
        assert_eq!(metadata.album, Some("Album".to_string()));
        assert_eq!(metadata.artist, "Artist".to_string());
        assert_eq!(metadata.album_artist, Some("AlbumArtist".to_string()));
        assert_eq!(metadata.date, Some("2021-12-02".to_string()));
        assert_eq!(metadata.track_number, Some(1));
        assert_eq!(metadata.disc_number, Some(1));
    }
}
