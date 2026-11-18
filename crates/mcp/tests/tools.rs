use jsoncv_core::library::Library;
use jsoncv_mcp::*;
use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult};
use serde_json::{json, Value};

fn server() -> (tempfile::TempDir, JsoncvServer) {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(dir.path()).unwrap();
    (dir, JsoncvServer::new(lib))
}

fn text(r: &CallToolResult) -> String {
    serde_json::to_value(&r.content[0]).unwrap()["text"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn update_cv_invalid_returns_field_errors() {
    let (_d, s) = server();
    let r = s
        .update_cv(Parameters(UpdateCvArgs {
            name: "main".into(),
            patch: vec![serde_json::from_value(
                json!({"op":"replace","path":"/work/0/startDate","value":"not a date"}),
            )
            .unwrap()],
            message: None,
        }))
        .await;
    assert_eq!(r.is_error, Some(true));
    let v: Value = serde_json::from_str(&text(&r)).unwrap();
    assert_eq!(v["errors"][0]["path"], "/work/0/startDate");
}

#[tokio::test]
async fn get_then_replace_roundtrip() {
    let (_d, s) = server();
    let r = s
        .get_cv(Parameters(NameArgs {
            name: "main".into(),
        }))
        .await;
    assert_eq!(r.is_error, Some(false));
    let v: Value = serde_json::from_str(&text(&r)).unwrap();
    let mut cv = v["cv"].clone();
    cv["basics"]["name"] = json!("Changed Name");
    let r = s
        .replace_cv(Parameters(ReplaceCvArgs {
            name: "main".into(),
            cv,
            base: v["base"].as_str().unwrap().into(),
            message: None,
        }))
        .await;
    assert_eq!(r.is_error, Some(false), "{}", text(&r));
    let r = s
        .get_cv(Parameters(NameArgs {
            name: "main".into(),
        }))
        .await;
    let v: Value = serde_json::from_str(&text(&r)).unwrap();
    assert_eq!(v["cv"]["basics"]["name"], "Changed Name");
}

#[tokio::test]
async fn save_theme_returns_report() {
    let (_d, s) = server();
    let r = s
        .save_theme(Parameters(SaveThemeArgs {
            id: "mine".into(),
            template: "<h1>{{ cv.basics.name }}</h1>".into(),
            css: "h1{}".into(),
            meta: serde_json::from_value(
                json!({"name":"Mine","description":"d","page":{"size":"A4"}}),
            )
            .unwrap(),
        }))
        .await;
    assert_eq!(r.is_error, Some(false), "{}", text(&r));
    let v: Value = serde_json::from_str(&text(&r)).unwrap();
    assert!(v["render_error"].is_null());
    assert!(v["missing"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn add_asset_needs_exactly_one_source() {
    let (_d, s) = server();
    let r = s
        .add_asset(Parameters(AddAssetArgs {
            name: "a.png".into(),
            path: None,
            base64: None,
        }))
        .await;
    assert_eq!(r.is_error, Some(true));
    let r = s
        .add_asset(Parameters(AddAssetArgs {
            name: "a.png".into(),
            path: None,
            base64: Some("aGk=".into()),
        }))
        .await;
    let v: Value = serde_json::from_str(&text(&r)).unwrap();
    assert_eq!(v["path"], "assets/a.png");
}

#[test]
fn tools_list_has_14_tools_with_exact_names() {
    let mut names: Vec<String> = JsoncvServer::tool_router()
        .list_all()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    names.sort();
    let mut want = vec![
        "list_cvs",
        "get_cv",
        "update_cv",
        "replace_cv",
        "create_tailored_cv",
        "cv_history",
        "restore_cv_version",
        "list_themes",
        "get_theme",
        "save_theme",
        "set_cv_theme",
        "preview",
        "export_cv",
        "add_asset",
    ];
    want.sort();
    assert_eq!(names, want);
}

fn schema_of(name: &str) -> Value {
    let t = JsoncvServer::tool_router()
        .list_all()
        .into_iter()
        .find(|t| t.name == name)
        .unwrap();
    serde_json::to_value(&*t.input_schema).unwrap()
}

#[test]
fn input_schemas_are_typed() {
    let s = schema_of("export_cv").to_string();
    for f in ["\"pdf\"", "\"html\"", "\"json\""] {
        assert!(s.contains(f), "{s}");
    }
    let p = schema_of("update_cv");
    assert!(p["$defs"]["PatchOp"]["properties"]["op"].is_object(), "{p}");
}

#[test]
fn resources_have_uri_mime_and_text() {
    for (uri, mime) in [
        ("jsoncv://schema", "application/json"),
        ("jsoncv://theme-guide", "text/markdown"),
        ("jsoncv://sample-cv", "application/json"),
    ] {
        let v = serde_json::to_value(resource_contents(uri).unwrap()).unwrap();
        assert_eq!(v["uri"], uri);
        assert_eq!(v["mimeType"], mime);
        assert!(!v["text"].as_str().unwrap().is_empty());
    }
    assert!(resource_contents("jsoncv://nope").is_none());
}
