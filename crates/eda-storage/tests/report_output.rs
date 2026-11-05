#[test]
fn reports_are_complete_and_never_replace_existing_files() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("report.json");
    eda_storage::write_new_report(&path, b"{\"ready\":true}\n").unwrap();
    assert!(eda_storage::write_new_report(&path, b"replacement").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"{\"ready\":true}\n");
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
fn failed_publication_leaves_no_temporary_report() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("directory");
    std::fs::create_dir(&destination).unwrap();
    assert!(eda_storage::write_new_report(&destination, b"report").is_err());
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
    assert!(std::fs::read_dir(destination).unwrap().next().is_none());
}
