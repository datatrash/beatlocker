#![allow(dead_code, unused)]

mod test_client;

use beatlocker_server::AppResult;
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use std::fs;
use std::path::Path;
pub use test_client::*;

pub const MOTORWAY_OST_FOLDER_UUID: &str = "68f8b71b-d9b4-c77e-c7f1-e4af263bcd93";
pub const MOTORWAY_OST_ALBUM_UUID: &str = "20d02390-2687-c407-2d28-d74f9fc6d5a1";
pub const MOTORWAY_OST_RADAR_UNIT_FOLDER_CHILD_UUID: &str = "9fe0fb24-dabd-4464-258b-1ab72a28aa94";
pub const MOTORWAY_OST_RADAR_UNIT_SONG_UUID: &str = "f417c310-98e2-e42f-ed0d-f9208c48419b";
pub const RICHARD_BONA_UUID: &str = "d094e9f8-a8e2-1737-0cfc-c4b24ab0aedf";

pub fn prettify_xml(xml: &str) -> String {
    let mut buf = Vec::new();

    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);

    loop {
        let ev = reader.read_event_into(&mut buf);

        match ev {
            Ok(Event::Eof) => break, // exits the loop when reaching end of file
            Ok(event) => writer.write_event(event),
            Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
        }
        .expect("Failed to parse XML");

        // If we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
        buf.clear();
    }

    let result = std::str::from_utf8(&writer.into_inner())
        .expect("Failed to convert a slice of bytes to a string slice")
        .to_string();

    result
}

pub fn copy_recursively(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> AppResult<()> {
    fs::create_dir_all(&destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let filetype = entry.file_type()?;
        if filetype.is_dir() {
            copy_recursively(entry.path(), destination.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), destination.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
