use jsoncv_core::{library::Library, Author};
use tempfile::TempDir;

fn setup() -> (TempDir, Library) {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    lib.create_cv("a", None, Author::You).unwrap();
    lib.create_cv("b", Some("jobs"), Author::You).unwrap();
    (d, lib)
}

fn keys(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn order_round_trips_and_follows_renames() {
    let (d, lib) = setup();
    assert!(lib.order().is_empty());
    lib.set_order(&keys(&["jobs/", "b", "a", "my-cv"])).unwrap();
    assert_eq!(lib.order(), keys(&["jobs/", "b", "a", "my-cv"]));
    assert!(d.path().join(".jsoncv-order.json").exists());
    lib.rename_cv("a", "c", Author::You).unwrap();
    lib.rename_folder("jobs", "work").unwrap();
    assert_eq!(lib.order(), keys(&["work/", "b", "c", "my-cv"]));
    lib.delete_cv("c", Author::You).unwrap();
    assert_eq!(lib.order(), keys(&["work/", "b", "my-cv"]));
}

#[test]
fn deleting_a_folder_drops_its_key() {
    let (_d, lib) = setup();
    lib.create_folder("empty").unwrap();
    lib.set_order(&keys(&["empty/", "a"])).unwrap();
    lib.delete_folder("empty").unwrap();
    assert_eq!(lib.order(), keys(&["a"]));
}

#[test]
fn corrupt_order_file_is_empty() {
    let (d, lib) = setup();
    std::fs::write(d.path().join(".jsoncv-order.json"), "nope").unwrap();
    assert!(lib.order().is_empty());
}

#[test]
fn set_pin_order_reorders_and_drops_unknown() {
    let (_d, lib) = setup();
    lib.set_pinned("a", true).unwrap();
    lib.set_pinned("b", true).unwrap();
    lib.set_pin_order(&keys(&["b", "gone", "a", "b"])).unwrap();
    let pin = |n: &str| {
        lib.list_cvs()
            .unwrap()
            .into_iter()
            .find(|c| c.name == n)
            .unwrap()
            .pin
    };
    assert_eq!((pin("b"), pin("a")), (Some(0), Some(1)));
}
