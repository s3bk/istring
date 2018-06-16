extern crate istring;
use istring::IString;

use std::thread;
use std::fmt::Write;

#[test]
fn test_thread() {
    let mut s = IString::from("Hello");
    write!(s, " world").unwrap();
    let s2 = thread::spawn(move || {
        let mut s = s;
        s += " from another thread!";
        s
    }).join().unwrap();
    assert_eq!(s2, "Hello world from another thread!");
}
