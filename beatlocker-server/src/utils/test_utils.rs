use crate::{App, AppResult, DatabaseOptions, Db, ServerOptions};
use chrono::{DateTime, Utc};
use id3::{Tag, TagLike, Timestamp};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use tempdir::TempDir;

pub struct TestState {
    pub app: App,
    pub tempdir: Option<TempDir>,
}

impl TestState {
    pub async fn new() -> AppResult<TestState> {
        let tempdir = Some(TempDir::new_in(".", "mock")?);
        add_mock_data(tempdir.as_ref().unwrap().path()).await?;

        let options = ServerOptions {
            path: tempdir.as_ref().unwrap().path().into(),
            database: DatabaseOptions {
                path: Some(PathBuf::from_str(".")?),
                in_memory: true,
            },
            now_provider: Arc::new(Box::new(|| {
                DateTime::parse_from_rfc3339("2020-02-02T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc)
            })),
            ..Default::default()
        };
        let app = App::new(options).await?;
        app.task_manager
            .send(app.import_all_folders().await?)
            .await?;

        Ok(TestState { app, tempdir })
    }

    pub async fn db(&self) -> Arc<Db> {
        self.app.state.read().await.db.clone()
    }
}

pub async fn add_mock_data(path: &Path) -> AppResult<()> {
    // Three folders
    // Three albums
    // Two artists and a 'Various Artists' album artist
    // Album 1 made by artist 1, released in 2025
    // Album 2 made by artist 2, released in 2020
    // Album 3 made by both artists, released in 2014, under "Various Artists"
    // Folder 1: 2 songs by artist 1 (album 1), 1 song by artist 2 (album 2)
    // Folder 2: 2 songs by artist 1 (album 1), 1 song by artist 2 (album 2)
    // Folder 3: 1 song by artist 1, 1 song by artist 2, both part of album 3
    fs::create_dir_all(path.join("folder1"))?;
    fs::create_dir_all(path.join("folder2"))?;
    fs::create_dir_all(path.join("folder3"))?;
    write_mp3(&path.join("folder1/artist1-a.mp3"), |tag| {
        tag.set_title("A");
        tag.set_album("Artist1_Album1");
        tag.set_artist("Artist1");
        tag.set_track(1);
        tag.set_disc(1);
        tag.set_genre("Genre1");
        tag.set_date_recorded(Timestamp::from_str("2025").unwrap());
    })?;
    write_mp3(&path.join("folder1/artist1-b.mp3"), |tag| {
        tag.set_title("B");
        tag.set_album("Artist1_Album1");
        tag.set_artist("Artist1");
        tag.set_track(2);
        tag.set_disc(1);
        tag.set_genre("Genre2");
        tag.set_date_recorded(Timestamp::from_str("2025").unwrap());
    })?;
    write_mp3(&path.join("folder1/artist2-c.mp3"), |tag| {
        tag.set_title("C");
        tag.set_album("Artist2_Album1");
        tag.set_artist("Artist2");
        tag.set_track(1);
        tag.set_disc(1);
        tag.set_genre("Genre3");
        tag.set_date_recorded(Timestamp::from_str("2020").unwrap());
    })?;
    write_mp3(&path.join("folder2/artist1-d.mp3"), |tag| {
        tag.set_title("D");
        tag.set_album("Artist1_Album1");
        tag.set_artist("Artist1");
        tag.set_track(3);
        tag.set_disc(1);
        tag.set_genre("Genre4");
        tag.set_date_recorded(Timestamp::from_str("2025").unwrap());
    })?;
    write_mp3(&path.join("folder2/artist1-e.mp3"), |tag| {
        tag.set_title("E");
        tag.set_album("Artist1_Album1");
        tag.set_artist("Artist1");
        tag.set_track(4);
        tag.set_disc(1);
        tag.set_genre("Genre5");
        tag.set_date_recorded(Timestamp::from_str("2025").unwrap());
    })?;
    write_mp3(&path.join("folder2/artist2-f.mp3"), |tag| {
        tag.set_title("F");
        tag.set_album("Artist2_Album1");
        tag.set_artist("Artist2");
        tag.set_track(2);
        tag.set_disc(1);
        tag.set_genre("Genre6");
        tag.set_date_recorded(Timestamp::from_str("2020").unwrap());
    })?;
    write_mp3(&path.join("folder3/artist1-g.mp3"), |tag| {
        tag.set_title("G");
        tag.set_album("SharedAlbum");
        tag.set_artist("Artist1");
        tag.set_album_artist("Various Artists");
        tag.set_track(11);
        tag.set_disc(1);
        tag.set_genre("Genre7");
        tag.set_date_recorded(Timestamp::from_str("2014").unwrap());
    })?;
    write_mp3(&path.join("folder3/artist2-h.mp3"), |tag| {
        tag.set_title("H");
        tag.set_album("SharedAlbum");
        tag.set_artist("Artist2");
        tag.set_album_artist("Various Artists");
        tag.set_track(12);
        tag.set_disc(1);
        tag.set_genre("Genre8");
        tag.set_date_recorded(Timestamp::from_str("2014").unwrap());
    })?;

    Ok(())
}

/// Leaks the inner TempDir if we are unwinding.
impl Drop for TestState {
    fn drop(&mut self) {
        if ::std::thread::panicking() {
            //::std::mem::forget(self.tempdir.take())
        }
    }
}
fn write_mp3(path: &Path, tag_fn: impl FnOnce(&mut Tag)) -> AppResult<()> {
    fs::write(path, include_bytes!("../../tests/silent.mp3"))?;
    let mut tag = Tag::new();
    tag_fn(&mut tag);
    tag.write_to_path(path, id3::Version::Id3v24)?;
    Ok(())
}
