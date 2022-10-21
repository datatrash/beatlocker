use crate::api::model::SubsonicAlbum;
use crate::{AppResult, Deserialize};
use axum::async_trait;
use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use axum::response::Response;
use chrono::{DateTime, Datelike, Utc};

use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row, SqliteConnection};
use uuid::Uuid;

pub struct GetSubsonicAlbumsQuery {
    pub album_id: Option<Uuid>,
    pub artist_id: Option<Uuid>,
    pub folder_id: Option<Uuid>,
    //pub music_folder_id: Option<Uuid>,
    pub offset: u32,
    pub size: u32,
    pub ty: GetSubsonicAlbumsListType,
    pub starred: bool,
}

#[derive(Debug, Deserialize)]
pub enum GetSubsonicAlbumsListType {
    Random,
    Newest,
    Recent,
    Starred,
    AlphabeticalByName,
    AlphabeticalByArtist,
    ByYear { from_year: usize, to_year: usize },
    ByGenre { genre: String },
}

impl Default for GetSubsonicAlbumsQuery {
    fn default() -> Self {
        Self {
            album_id: None,
            artist_id: None,
            folder_id: None,
            //music_folder_id: None,
            size: 20,
            offset: 0,
            ty: GetSubsonicAlbumsListType::AlphabeticalByName,
            starred: false,
        }
    }
}

pub async fn get_subsonic_albums(
    conn: &mut SqliteConnection,
    query: GetSubsonicAlbumsQuery,
) -> AppResult<Vec<SubsonicAlbum>> {
    let mut builder = QueryBuilder::new(
        r#"SELECT f.*, MIN(s.date) AS song_date, COUNT(fc.song_id) AS song_count, SUM(s.duration) AS duration, st.created as starred_date
        FROM folders f
        LEFT JOIN folder_children fc on f.folder_id = fc.folder_id
        LEFT JOIN songs s on fc.song_id = s.song_id
        LEFT JOIN starred st ON st.starred_id = fc.folder_id
        "#,
    );

    builder.push(" WHERE f.parent_id IS NOT NULL");
    /*if let Some(id) = query.music_folder_id {
        builder.push(" AND f.folder_id = ").push_bind(id);
    };*/
    if let Some(id) = query.album_id {
        builder.push(" AND s.album_id = ").push_bind(id);
    };
    if let Some(id) = query.artist_id {
        builder.push(" AND s.artist_id = ").push_bind(id);
    };
    if query.starred {
        builder.push(" AND starred_date IS NOT NULL");
    }

    if let GetSubsonicAlbumsListType::ByYear { from_year, to_year } = query.ty {
        let mut from_year: DateTime<Utc> = DateTime::default().with_year(from_year as i32).unwrap();
        let mut to_year: DateTime<Utc> = DateTime::default().with_year(to_year as i32).unwrap();
        if from_year > to_year {
            std::mem::swap(&mut from_year, &mut to_year);
        }
        builder
            .push(" AND s.date >= ")
            .push_bind(from_year)
            .push(" AND s.date <= ")
            .push_bind(to_year);
    }

    if let GetSubsonicAlbumsListType::ByGenre { ref genre } = query.ty {
        builder.push(" AND s.genre = ").push_bind(genre);
    }

    builder.push(" GROUP BY 1");

    match query.ty {
        GetSubsonicAlbumsListType::Random => (),
        GetSubsonicAlbumsListType::Newest => {
            builder.push(" ORDER BY folders.created DESC");
        }
        GetSubsonicAlbumsListType::Recent => {
            builder.push(" ORDER BY folders.created DESC");
        }
        GetSubsonicAlbumsListType::Starred => (),
        GetSubsonicAlbumsListType::AlphabeticalByName => {
            builder.push(" ORDER BY title");
        }
        GetSubsonicAlbumsListType::AlphabeticalByArtist => {
            builder.push(" ORDER BY title");
        }
        GetSubsonicAlbumsListType::ByYear { from_year, to_year } => {
            builder.push(" ORDER BY song_date");
            if from_year > to_year {
                builder.push(" DESC");
            }
            builder.push(", title");
        }
        GetSubsonicAlbumsListType::ByGenre { .. } => (),
    };

    builder
        .push(" LIMIT ")
        .push_bind(query.offset)
        .push(", ")
        .push_bind(query.size);

    let artists = builder
        .build()
        .map(|row: SqliteRow| {
            let id: Uuid = row.get("folder_id");
            SubsonicAlbum {
                id,
                parent: Some(Uuid::nil()),
                is_dir: Some(true),
                name: row.get("name"),
                title: row.get("name"),
                song_count: row.get("song_count"),
                duration: row.get("duration"),
                starred: row.get("starred_date"),
                ..Default::default()
            }
        })
        .fetch_all(conn)
        .await
        .unwrap();

    Ok(artists)
}

pub async fn get_subsonic_albums_by_id3(
    conn: &mut SqliteConnection,
    query: GetSubsonicAlbumsQuery,
) -> AppResult<Vec<SubsonicAlbum>> {
    let mut builder = QueryBuilder::new(
        r#"
        SELECT albums.*, ar.name AS artist_name, ar.artist_id AS artist_id, MIN(s.date) AS song_date, COUNT(s.song_id) AS song_count, SUM(s.duration) AS duration, st.created as starred_date
        FROM albums
        LEFT JOIN album_artists aa on albums.album_id = aa.album_id
        LEFT JOIN artists ar on aa.artist_id = ar.artist_id
        LEFT JOIN songs s on s.album_id = albums.album_id
        LEFT JOIN starred st ON st.starred_id = albums.album_id
        "#,
    );

    builder.push(" WHERE 1=1");
    if let Some(id) = query.folder_id {
        builder.push(" AND folder_id = ").push_bind(id);
    };
    if let Some(id) = query.album_id {
        builder.push(" AND albums.album_id = ").push_bind(id);
    };
    if let Some(id) = query.artist_id {
        builder.push(" AND aa.artist_id = ").push_bind(id);
    };
    if query.starred {
        builder.push(" AND starred_date IS NOT NULL");
    }

    if let GetSubsonicAlbumsListType::ByYear { from_year, to_year } = query.ty {
        let mut from_year: DateTime<Utc> = DateTime::default().with_year(from_year as i32).unwrap();
        let mut to_year: DateTime<Utc> = DateTime::default().with_year(to_year as i32).unwrap();
        if from_year > to_year {
            std::mem::swap(&mut from_year, &mut to_year);
        }
        builder
            .push(" AND s.date >= ")
            .push_bind(from_year)
            .push(" AND s.date <= ")
            .push_bind(to_year);
    }

    if let GetSubsonicAlbumsListType::ByGenre { ref genre } = query.ty {
        builder.push(" AND s.genre = ").push_bind(genre);
    }

    builder.push(" GROUP BY 1");

    match query.ty {
        GetSubsonicAlbumsListType::Random => (),
        GetSubsonicAlbumsListType::Newest => {
            builder.push(" ORDER BY s.created DESC");
        }
        GetSubsonicAlbumsListType::Recent => {
            builder.push(" ORDER BY s.created DESC");
        }
        GetSubsonicAlbumsListType::Starred => (),
        GetSubsonicAlbumsListType::AlphabeticalByName => {
            builder.push(" ORDER BY albums.title");
        }
        GetSubsonicAlbumsListType::AlphabeticalByArtist => {
            builder.push(" ORDER BY artist_name, albums.title");
        }
        GetSubsonicAlbumsListType::ByYear { from_year, to_year } => {
            builder.push(" ORDER BY song_date");
            if from_year > to_year {
                builder.push(" DESC");
            }
            builder.push(", title");
        }
        GetSubsonicAlbumsListType::ByGenre { .. } => (),
    };

    builder
        .push(" LIMIT ")
        .push_bind(query.offset)
        .push(", ")
        .push_bind(query.size);

    let artists = builder
        .build()
        .map(|row: SqliteRow| {
            let id: Uuid = row.get("album_id");
            SubsonicAlbum {
                id,
                name: row.get("title"),
                title: row.get("title"),
                song_count: row.get("song_count"),
                duration: row.get("duration"),
                artist: row.get("artist_name"),
                artist_id: row.get("artist_id"),
                cover_art: row.get("cover_art_id"),
                starred: row.get("starred_date"),
                ..Default::default()
            }
        })
        .fetch_all(conn)
        .await
        .unwrap();

    Ok(artists)
}

#[async_trait]
impl<S> FromRequestParts<S> for GetSubsonicAlbumsListType
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Query::<GetSubsonicAlbumsListTypeQuery>::from_request_parts(parts, state)
            .await
            .ok()
        {
            Some(query) => match (&query.ty, query.from_year, query.to_year, &query.genre) {
                (Some(ty), _, _, _) if ty == "alphabeticalByName" => {
                    Ok(GetSubsonicAlbumsListType::AlphabeticalByName)
                }
                (Some(ty), _, _, _) if ty == "alphabeticalByArtist" => {
                    Ok(GetSubsonicAlbumsListType::AlphabeticalByArtist)
                }
                (Some(ty), _, _, _) if ty == "random" => Ok(GetSubsonicAlbumsListType::Random),
                (Some(ty), _, _, _) if ty == "recent" => Ok(GetSubsonicAlbumsListType::Recent),
                (Some(ty), _, _, _) if ty == "starred" => Ok(GetSubsonicAlbumsListType::Starred),
                (Some(ty), _, _, _) if ty == "newest" => Ok(GetSubsonicAlbumsListType::Newest),
                (Some(ty), Some(from_year), Some(to_year), _) if ty == "byYear" => {
                    Ok(GetSubsonicAlbumsListType::ByYear { from_year, to_year })
                }
                (Some(ty), _, _, Some(genre)) if ty == "byGenre" => {
                    Ok(GetSubsonicAlbumsListType::ByGenre {
                        genre: genre.clone(),
                    })
                }
                _ => Ok(GetSubsonicAlbumsListType::AlphabeticalByName),
            },
            None => Ok(GetSubsonicAlbumsListType::AlphabeticalByName),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetSubsonicAlbumsListTypeQuery {
    #[serde(rename = "type")]
    ty: Option<String>,
    from_year: Option<usize>,
    to_year: Option<usize>,
    genre: Option<String>,
}
