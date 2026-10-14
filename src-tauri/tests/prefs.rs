extern crate dragontail_lib as dragontail;

#[allow(clippy::unwrap_used)]
#[test]
fn rejects_unknown_key() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = dragontail::prefs::open(&dir.path().join("t.sqlite")).unwrap();
    let err = prefs.set("dragontail-window-frame-v1", serde_json::json!({"x": 1}));
    assert!(err.is_err());
}

#[allow(clippy::unwrap_used)]
#[test]
fn round_trips_onboarding() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = dragontail::prefs::open(&dir.path().join("t.sqlite")).unwrap();
    prefs
        .set(
            "dragontail-onboarding-v1",
            serde_json::json!({"completed": true}),
        )
        .unwrap();
    let value = prefs.get("dragontail-onboarding-v1").unwrap().unwrap();
    assert_eq!(value["completed"], true);
}
