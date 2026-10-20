use covenant_knowledge::{CompileOptions, Store};
use serde_json::json;
use std::sync::{Arc, Barrier};

fn concurrent_upgrade(version: i64) {
    let source = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let mut text = String::from("def base():\n    return 1\n\n");
    // A nonempty backfill must survive the migration, not only its schema.
    for index in 0..200 {
        text.push_str(&format!("def caller_{index}():\n    return base()\n\n"));
    }
    std::fs::write(source.path().join("example.py"), text).unwrap();
    let store = Store::open(home.path()).unwrap();
    let compiled = store
        .compile(source.path(), CompileOptions::default())
        .unwrap();
    let project = compiled["project"]["id"].as_str().unwrap().to_string();
    let snapshot = store.snapshot(&project, None).unwrap();
    let object = snapshot
        .objects
        .iter()
        .find(|object| object.name == "base" && object.kind == "function")
        .unwrap()
        .id
        .clone();
    let expected = store
        .query(
            &project,
            None,
            "dependencies",
            &json!({"id":object,"direction":"incoming"}),
        )
        .unwrap();
    assert!(!expected["relationships"].as_array().unwrap().is_empty());
    drop(store);
    let database = home.path().join("knowledge.sqlite3");
    {
        let db = rusqlite::Connection::open(&database).unwrap();
        db.execute_batch("DROP TABLE relationship_index;").unwrap();
        if version == 1 {
            db.execute_batch("ALTER TABLE parse_cache DROP COLUMN checksum;")
                .unwrap();
        }
        db.pragma_update(None, "user_version", version).unwrap();
    }

    let barrier = Arc::new(Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let barrier = barrier.clone();
            let path = home.path().to_path_buf();
            let project = project.clone();
            let object = object.clone();
            let snapshot_id = snapshot.id.clone();
            let expected = expected.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let store = Store::open(path).unwrap();
                assert_eq!(store.snapshot(&project, None).unwrap().id, snapshot_id);
                assert_eq!(
                    store
                        .query(
                            &project,
                            None,
                            "dependencies",
                            &json!({"id":object,"direction":"incoming"})
                        )
                        .unwrap(),
                    expected
                );
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let db = rusqlite::Connection::open(database).unwrap();
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM relationship_index", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap(),
        snapshot.relationships.len()
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM events", [], |row| row
            .get::<_, usize>(0))
            .unwrap(),
        1
    );
}

#[test]
fn concurrent_open_upgrades_v1_without_losing_snapshot_or_relationships() {
    concurrent_upgrade(1);
}

#[test]
fn concurrent_open_upgrades_v2_without_losing_snapshot_or_relationships() {
    concurrent_upgrade(2);
}

#[test]
fn concurrent_open_initializes_one_fresh_store() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("new-store");
    let barrier = Arc::new(Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                assert!(Store::open(path).unwrap().projects().unwrap().is_empty());
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let db = rusqlite::Connection::open(path.join("knowledge.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        3
    );
}
