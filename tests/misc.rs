extern crate istring;
use istring::IString;

#[test]
fn test_misc() {
    let p1 = "Hello World!";
    let p2 = "Hello World! .........xyz";
    let p3 = " .........xyz";
    
    let s1 = IString::from(p1);
    assert_eq!(s1, p1);
    
    let s2 = IString::from(p2);
    assert_eq!(s2, p2);
    
    let mut s3 = s1.clone();
    s3.push_str(p3);
    assert_eq!(s3, p2);
}
