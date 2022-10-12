CREATE TABLE albums
(
    album_id text primary key not null,
    uri text not null unique,
    title text not null,
    cover_art_id text,
    foreign key (cover_art_id) references cover_art(cover_art_id)
);

CREATE TABLE folders
(
    folder_id text primary key not null,
    parent_id text,
    uri text not null unique,
    name text not null,
    cover_art_id text,
    created datetime not null,
    foreign key (parent_id) references folders(folder_id),
    foreign key (cover_art_id) references cover_art(cover_art_id)
);

CREATE TABLE folder_children
(
    folder_child_id text primary key not null,
    folder_id text not null,
    uri text not null unique,
    path text not null,
    name text not null,
    song_id text,
    foreign key (folder_id) references folders(folder_id),
    foreign key (song_id) references songs(song_id)
);

CREATE TABLE cover_art
(
    cover_art_id text primary key not null,
    uri text not null unique,
    data blob
);

CREATE TABLE artists
(
    artist_id text primary key not null,
    uri text not null unique,
    name text not null,
    cover_art_id number,
    foreign key (cover_art_id) references cover_art(cover_art_id)
);

CREATE TABLE album_artists
(
    album_id text not null,
    artist_id text not null,
    foreign key (artist_id) references artists(artist_id),
    foreign key (album_id) references albums(album_id),
    primary key (album_id, artist_id)
);

CREATE TABLE songs
(
    song_id text primary key not null,
    uri text not null unique,
    title text not null,
    created datetime not null,
    date datetime,
    cover_art_id text,
    artist_id text,
    album_id text,
    content_type text,
    suffix text,
    size number,
    track_number number,
    disc_number number,
    duration number,
    bit_rate number,
    genre text,
    foreign key (cover_art_id) references cover_art(cover_art_id),
    foreign key (artist_id) references artists(artist_id),
    foreign key (album_id) references albums(album_id)
);
