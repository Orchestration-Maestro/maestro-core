//! A manifest is read one line at a time, each numbered from 1: a line that
//! is not UTF-8 is refused on its own, the text after the last newline is a
//! line of its own, and once the manifest ends there is no next line.

use crate::import::{
    Reason,
    source::{Line, Lines},
};

#[test]
fn a_manifest_is_read_one_numbered_line_at_a_time_until_it_ends() {
    let mut lines = Lines::new(&b"first\n\nthird\r\n\xff\xfe\nlast"[..]);
    let mut read = Vec::new();
    for _ in 0..7 {
        let next = lines.next().unwrap();
        read.push(next.map(|Line { number, text }| (number, text.map(str::to_owned))));
    }
    let [first, empty, third, binary, last, ended, still] = read.as_slice() else {
        panic!("{read:?}");
    };
    assert_eq!(first, &Some((1, Ok("first".to_owned()))));
    assert_eq!(empty, &Some((2, Ok(String::new()))));
    assert_eq!(
        third,
        &Some((3, Ok("third\r".to_owned()))),
        "JSON reads the CR as space"
    );
    assert!(
        matches!(binary, Some((4, Err(Reason::Malformed { message })))
            if message.starts_with("the line is not UTF-8")),
        "{binary:?}"
    );
    assert_eq!(last, &Some((5, Ok("last".to_owned()))));
    assert_eq!((ended, still), (&None, &None));
}
