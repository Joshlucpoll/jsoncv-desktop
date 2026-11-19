use jsoncv_core::{library::Library, Author, CoreError};
use tempfile::TempDir;

#[test]
fn add_asset_dedupes_and_suffixes() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    assert_eq!(
        lib.add_asset("my photo!.png", b"aaa", Author::You).unwrap(),
        "assets/myphoto.png"
    );
    assert_eq!(
        lib.add_asset("myphoto.png", b"aaa", Author::You).unwrap(),
        "assets/myphoto.png"
    );
    assert_eq!(
        lib.add_asset("myphoto.png", b"bbb", Author::You).unwrap(),
        "assets/myphoto-2.png"
    );
    assert_eq!(
        lib.add_asset("../myphoto.png", b"ccc", Author::You)
            .unwrap(),
        "assets/myphoto-3.png"
    );
    assert_eq!(
        std::fs::read(d.path().join("assets/myphoto-2.png")).unwrap(),
        b"bbb"
    );
    assert!(matches!(
        lib.add_asset("***", b"x", Author::You),
        Err(CoreError::BadPath(_))
    ));
    let big = vec![0u8; 20 * 1024 * 1024 + 1];
    assert!(matches!(
        lib.add_asset("b.bin", &big, Author::You),
        Err(CoreError::BadPath(_))
    ));
}
