#![forbid(unsafe_code)]
//! CLI tests for local `--db` (`site`, `key`, `import`, `backup`, `doctor`).

#![cfg(feature = "sqlite")]

use std::path::Path;
use std::process::ExitCode;

use rotameter_cli::exit::{EXIT_OK, EXIT_USAGE};
use rotameter_cli::run;

#[test]
fn local_db_lifecycle_site_key_import_backup_and_doctor() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("rotameter.db");
    let db_str = db.to_str().unwrap();

    assert_site_and_key_ops(db_str);
    assert_import_backup_doctor_purge_delete(db_str, dir.path()).unwrap();
}

fn assert_site_and_key_ops(db_str: &str) {
    assert_eq!(
        run([
            "rotameter",
            "site",
            "upsert",
            "colinwolff.dev",
            "--name",
            "Colin",
            "--db",
            db_str
        ]),
        ExitCode::from(EXIT_OK)
    );
    assert_eq!(
        run([
            "rotameter",
            "site",
            "list",
            "--db",
            db_str,
            "--format",
            "json"
        ]),
        ExitCode::from(EXIT_OK)
    );
    assert_eq!(
        run([
            "rotameter",
            "site",
            "show",
            "colinwolff.dev",
            "--db",
            db_str,
            "--format",
            "csv"
        ]),
        ExitCode::from(EXIT_OK)
    );
    assert_eq!(
        run([
            "rotameter",
            "key",
            "create",
            "--scope",
            "read",
            "--site",
            "colinwolff.dev",
            "--label",
            "dash",
            "--db",
            db_str
        ]),
        ExitCode::from(EXIT_OK)
    );
    assert_eq!(
        run(["rotameter", "key", "list", "--db", db_str]),
        ExitCode::from(EXIT_OK)
    );
}

fn assert_import_backup_doctor_purge_delete(db_str: &str, dir: &Path) -> std::io::Result<()> {
    let backup = dir.join("backup.db");
    let jsonl = dir.join("hits.jsonl");
    let clf = dir.join("access.log");
    std::fs::write(
        &jsonl,
        "{\"url\":\"/projects/rotameter\",\"ts\":\"2026-11-22T10:00:00Z\",\"visitor_hint\":\"u1\"}\nnot-json\n",
    )?;
    std::fs::write(
        &clf,
        "198.51.100.7 - - [22/Nov/2026:10:05:00 +0000] \"GET /about HTTP/1.1\" 200 512 \"https://news.ycombinator.com/\" \"Mozilla/5.0\"\n",
    )?;

    let jsonl_str = jsonl.to_string_lossy();
    let clf_str = clf.to_string_lossy();
    let backup_str = backup.to_string_lossy();

    assert_eq!(
        run([
            "rotameter",
            "import",
            "colinwolff.dev",
            &jsonl_str,
            "--dry-run",
            "--format",
            "json"
        ]),
        ExitCode::from(EXIT_OK)
    );
    assert_eq!(
        run([
            "rotameter",
            "import",
            "colinwolff.dev",
            &clf_str,
            "--db",
            db_str
        ]),
        ExitCode::from(EXIT_OK)
    );
    assert_eq!(
        run(["rotameter", "backup", &backup_str, "--db", db_str]),
        ExitCode::from(EXIT_OK)
    );
    assert!(backup.exists());
    assert_eq!(
        run(["rotameter", "doctor", "--local", "--db", db_str]),
        ExitCode::from(EXIT_OK)
    );
    assert_eq!(
        run([
            "rotameter",
            "site",
            "purge",
            "colinwolff.dev",
            "--before",
            "2030-01-01",
            "--db",
            db_str
        ]),
        ExitCode::from(EXIT_OK)
    );
    assert_eq!(
        run([
            "rotameter",
            "site",
            "delete",
            "colinwolff.dev",
            "--db",
            db_str
        ]),
        ExitCode::from(EXIT_USAGE)
    );
    assert_eq!(
        run([
            "rotameter",
            "site",
            "delete",
            "colinwolff.dev",
            "--confirm",
            "--db",
            db_str
        ]),
        ExitCode::from(EXIT_OK)
    );
    Ok(())
}
