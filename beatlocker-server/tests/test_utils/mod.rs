mod test_client;

use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
pub use test_client::*;

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
