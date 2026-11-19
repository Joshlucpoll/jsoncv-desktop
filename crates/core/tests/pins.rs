use jsoncv_core::{library::Library, Author, CoreError};
use tempfile::TempDir;

fn setup() -> (TempDir, Library) {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    lib.create_cv("a", None, Author::You).unwrap();
    lib.create_cv("b", None, Author::You).unwrap();
    (d, lib)
}

fn pin(lib: &Library, name: &str) -> Option<usize> {
    lib.list_cvs()
        .unwrap()
        .into_iter()
        .find(|c| c.name == name)
        .unwrap()
        .pin
}

#[test]
fn pin_order_is_file_order() {
    let (d, lib) = setup();
    lib.set_pinned("b", true).unwrap();
    lib.set_pinned("a", true).unwrap();
    assert_eq!((pin(&lib, "b"), pin(&lib, "a")), (Some(0), Some(1)));
    assert_eq!(pin(&lib, "my-cv"), None);
    let file = std::fs::read_to_string(d.path().join(".jsoncv-pins.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&file).unwrap(),
        ["b", "a"]
    );
    lib.set_pinned("b", false).unwrap();
    assert_eq!((pin(&lib, "b"), pin(&lib, "a")), (None, Some(0)));
    // Pinning twice keeps one entry.
    lib.set_pinned("a", true).unwrap();
    assert_eq!(pin(&lib, "a"), Some(0));
    assert!(matches!(
        lib.set_pinned("nope", true),
        Err(CoreError::NotFound(_))
    ));
}

#[test]
fn pin_survives_rename_and_move() {
    let (_d, lib) = setup();
    lib.set_pinned("a", true).unwrap();
    lib.rename_cv("a", "c", Author::You).unwrap();
    assert_eq!(pin(&lib, "c"), Some(0));
    lib.move_cv("c", Some("jobs")).unwrap();
    assert_eq!(pin(&lib, "c"), Some(0));
}

#[test]
fn pin_dropped_on_delete() {
    let (d, lib) = setup();
    lib.set_pinned("a", true).unwrap();
    lib.delete_cv("a", Author::You).unwrap();
    let file = std::fs::read_to_string(d.path().join(".jsoncv-pins.json")).unwrap();
    assert!(serde_json::from_str::<Vec<String>>(&file)
        .unwrap()
        .is_empty());
    // Recreating the slug does not bring the pin back.
    lib.create_cv("a", None, Author::You).unwrap();
    assert_eq!(pin(&lib, "a"), None);
}

#[test]
fn corrupt_pins_file_is_empty() {
    let (d, lib) = setup();
    std::fs::write(d.path().join(".jsoncv-pins.json"), "{nope").unwrap();
    assert_eq!(pin(&lib, "a"), None);
    lib.set_pinned("a", true).unwrap();
    assert_eq!(pin(&lib, "a"), Some(0));
}
