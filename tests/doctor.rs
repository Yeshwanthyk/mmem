use mmem::doctor::run_doctor;

#[test]
fn doctor_reports_missing_db() {
    let root = tempfile::tempdir().expect("root");
    let db_path = root.path().join("missing.sqlite");

    let report = run_doctor(&db_path, root.path());
    assert!(report.root_exists);
    assert!(!report.db_exists);
    assert!(!report.schema_ok);
    assert!(report.schema_error.is_none());
    assert_eq!(report.indexed_sessions, 0);
}
