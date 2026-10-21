use crate::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub root: String,
    pub latest: Option<String>,
}

pub struct Store {
    db: Connection,
    home: PathBuf,
}

fn valid_id(id: &str) -> Result<()> {
    ensure!(
        id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid knowledge identifier"
    );
    Ok(())
}

impl Store {
    pub fn open(home: impl AsRef<Path>) -> Result<Self> {
        let home = home.as_ref();
        fs::create_dir_all(home)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(home, fs::Permissions::from_mode(0o700))?;
        }
        let path = home.join("knowledge.sqlite3");
        ensure!(
            !path
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink()),
            "store database cannot be a symlink"
        );
        let db = Connection::open(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }
        db.busy_timeout(Duration::from_secs(10))?;
        // The first WAL transition can return SQLITE_BUSY without invoking
        // SQLite's busy handler when several new connections initialize at once.
        // Retry that idempotent transition before taking the migration lock.
        for attempt in 0..8 {
            match db.execute_batch(
                "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
            ) {
                Ok(()) => break,
                Err(rusqlite::Error::SqliteFailure(error, _))
                    if matches!(
                        error.code,
                        rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                    ) && attempt < 7 =>
                {
                    std::thread::sleep(Duration::from_millis(10 << attempt));
                }
                Err(error) => return Err(error).context("configure knowledge journal"),
            }
        }
        db.execute_batch("BEGIN IMMEDIATE;")?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        ensure!(
            version <= 3,
            "knowledge store schema is newer than this compiler; upgrade without deleting data"
        );
        if version == 0 {
            db.execute_batch("
                CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY,root TEXT NOT NULL UNIQUE,latest TEXT);
                CREATE TABLE IF NOT EXISTS snapshots(id TEXT PRIMARY KEY,artifact TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS project_snapshots(project TEXT NOT NULL REFERENCES projects(id),
                    snapshot TEXT NOT NULL REFERENCES snapshots(id),sequence INTEGER NOT NULL,
                    PRIMARY KEY(project,snapshot));
                CREATE TABLE IF NOT EXISTS events(sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                    project TEXT NOT NULL,kind TEXT NOT NULL,snapshot TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS parse_cache(key TEXT PRIMARY KEY,artifact TEXT NOT NULL,checksum TEXT NOT NULL);
                CREATE VIRTUAL TABLE IF NOT EXISTS object_search USING fts5(
                    snapshot UNINDEXED,id UNINDEXED,name,path,body,tokenize='unicode61');
                CREATE TRIGGER IF NOT EXISTS immutable_snapshots_update BEFORE UPDATE ON snapshots
                    BEGIN SELECT RAISE(ABORT,'snapshots are immutable'); END;
                CREATE TRIGGER IF NOT EXISTS immutable_snapshots_delete BEFORE DELETE ON snapshots
                    BEGIN SELECT RAISE(ABORT,'snapshots are immutable'); END;
                CREATE TRIGGER IF NOT EXISTS immutable_events_update BEFORE UPDATE ON events
                    BEGIN SELECT RAISE(ABORT,'events are append-only'); END;
                CREATE TRIGGER IF NOT EXISTS immutable_events_delete BEFORE DELETE ON events
                    BEGIN SELECT RAISE(ABORT,'events are append-only'); END;
                PRAGMA user_version=2;
            ")?;
        } else if version == 1 {
            db.execute_batch(
                "                ALTER TABLE parse_cache ADD COLUMN checksum TEXT NOT NULL DEFAULT '';
                PRAGMA user_version=2;
                ",
            )?;
        }
        if version < 3 {
            db.execute_batch(
                "                CREATE TABLE relationship_index(
                    snapshot TEXT NOT NULL, ordinal INTEGER NOT NULL,
                    from_id TEXT NOT NULL, to_id TEXT,
                    PRIMARY KEY(snapshot,ordinal));
                CREATE INDEX relationships_outgoing ON relationship_index(snapshot,from_id);
                CREATE INDEX relationships_incoming ON relationship_index(snapshot,to_id);
                INSERT INTO relationship_index(snapshot,ordinal,from_id,to_id)
                    SELECT s.id,j.key,json_extract(j.value,'$.from'),json_extract(j.value,'$.to')
                    FROM snapshots s,json_each(s.artifact,'$.relationships') j;
                PRAGMA user_version=3;
                ",
            )?;
        }
        db.execute_batch("COMMIT;")?;
        Ok(Self {
            db,
            home: home.canonicalize()?,
        })
    }

    pub fn compile(&self, root: &Path, options: CompileOptions) -> Result<Value> {
        let root = root.canonicalize()?;
        ensure!(!self.home.starts_with(&root),
            "knowledge store must be outside the source directory to avoid indexing its own database");
        let source = capture_source(&root, &options)?;
        let mut cache = BTreeMap::new();
        if !options.force_full {
            for file in &source.files {
                let key = parser::cache_key(file);
                let value: Option<(String, String)> = self
                    .db
                    .query_row(
                        "SELECT artifact,checksum FROM parse_cache WHERE key=?1",
                        [&key],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                if let Some((value, checksum)) = value {
                    // Corrupt cache is expendable, immutable source snapshots are not.
                    if sha256(value.as_bytes()) == checksum {
                        if let Ok(parsed) = serde_json::from_str::<parser::ParsedFile>(&value) {
                            if parsed.valid_for(file) {
                                cache.insert(key, parsed);
                            }
                        }
                    }
                }
            }
        }
        let (snapshot, reused, fresh) = compile_cached(source, &cache)?;
        let mut result = self.save(&root, &snapshot)?;
        result["reused_files"] = json!(reused);
        // Parsing cache is an optimization outside the snapshot transaction.
        for (key, parsed) in fresh {
            let artifact = serde_json::to_string(&parsed)?;
            self.db.execute(
                "INSERT OR REPLACE INTO parse_cache(key,artifact,checksum) VALUES(?1,?2,?3)",
                params![key, artifact, sha256(artifact.as_bytes())],
            )?;
        }
        Ok(result)
    }

    pub fn import_snapshot(&self, root: &Path, snapshot: Snapshot) -> Result<Value> {
        validate_snapshot(&snapshot)?;
        self.save(&root.canonicalize()?, &snapshot)
    }

    fn save(&self, root: &Path, snapshot: &Snapshot) -> Result<Value> {
        let root = root.to_str().context("non-UTF8 project path unsupported")?;
        let project_id = sha256(root.as_bytes());
        let artifact = serde_json::to_string(snapshot)?;
        let tx = self.db.unchecked_transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO projects(id,root) VALUES(?1,?2)",
            params![project_id, root],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO snapshots(id,artifact) VALUES(?1,?2)",
            params![snapshot.id, artifact],
        )?;
        let existing: String = tx.query_row(
            "SELECT artifact FROM snapshots WHERE id=?1",
            [&snapshot.id],
            |r| r.get(0),
        )?;
        ensure!(existing == artifact, "immutable snapshot collision");
        let previous: Option<String> = tx.query_row(
            "SELECT latest FROM projects WHERE id=?1",
            [&project_id],
            |r| r.get(0),
        )?;
        if previous.as_deref() != Some(&snapshot.id) {
            tx.execute(
                "INSERT INTO events(project,kind,snapshot) VALUES(?1,'snapshot',?2)",
                params![project_id, snapshot.id],
            )?;
            let sequence = tx.last_insert_rowid();
            tx.execute("INSERT OR IGNORE INTO project_snapshots(project,snapshot,sequence) VALUES(?1,?2,?3)",
                params![project_id,snapshot.id,sequence])?;
            tx.execute(
                "UPDATE projects SET latest=?1 WHERE id=?2",
                params![snapshot.id, project_id],
            )?;
        }
        rebuild(&tx, snapshot)?;
        tx.commit()?;
        Ok(
            json!({"project":Project {id:project_id,root:root.into(),latest:Some(snapshot.id.clone())},
            "snapshot_id":snapshot.id,"coverage":coverage(snapshot),"diagnostics":snapshot.diagnostics,
            "reused_files":0}),
        )
    }

    pub fn resolve_project(&self, selector: Option<&str>, cwd: &Path) -> Result<Project> {
        let row = |r: &rusqlite::Row<'_>| {
            Ok(Project {
                id: r.get(0)?,
                root: r.get(1)?,
                latest: r.get(2)?,
            })
        };
        if let Some(id) = selector {
            valid_id(id)?;
            return self
                .db
                .query_row("SELECT id,root,latest FROM projects WHERE id=?1", [id], row)
                .optional()?
                .context("unknown project");
        }
        let cwd = cwd.canonicalize()?;
        let mut candidates = self
            .projects()?
            .into_iter()
            .filter(|p| cwd.starts_with(&p.root))
            .collect::<Vec<_>>();
        candidates.sort_by_key(|p| std::cmp::Reverse(p.root.len()));
        candidates
            .into_iter()
            .next()
            .context("no project for this directory; select --project <id> or compile a repository")
    }

    pub fn projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self
            .db
            .prepare("SELECT id,root,latest FROM projects ORDER BY root")?;
        let rows = stmt.query_map([], |r| {
            Ok(Project {
                id: r.get(0)?,
                root: r.get(1)?,
                latest: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn snapshot(&self, project: &str, id: Option<&str>) -> Result<Snapshot> {
        let p = self.resolve_project(Some(project), Path::new("."))?;
        let id = id
            .or(p.latest.as_deref())
            .context("project has no snapshot")?;
        valid_id(id)?;
        let data: Option<String> = self
            .db
            .query_row(
                "SELECT s.artifact FROM snapshots s JOIN project_snapshots p ON p.snapshot=s.id
             WHERE p.project=?1 AND s.id=?2",
                params![project, id],
                |r| r.get(0),
            )
            .optional()?;
        let snapshot: Snapshot =
            serde_json::from_str(&data.context("snapshot unavailable in this project")?)?;
        ensure!(
            snapshot.id == id
                && snapshot.schema_version == SCHEMA_VERSION
                && snapshot_digest(&snapshot)? == id,
            "stored snapshot integrity check failed"
        );
        Ok(snapshot)
    }

    pub fn rebuild_indexes(&self, project: &str, id: Option<&str>) -> Result<()> {
        let snapshot = self.snapshot(project, id)?;
        validate_snapshot(&snapshot)?;
        let tx = self.db.unchecked_transaction()?;
        rebuild(&tx, &snapshot)?;
        tx.commit()?;
        Ok(())
    }

    pub fn query(
        &self,
        project: &str,
        snapshot_id: Option<&str>,
        operation: &str,
        args: &Value,
    ) -> Result<Value> {
        let project_record = self.resolve_project(Some(project), Path::new("."))?;
        if operation == "changes" {
            let from = self.snapshot(project, Some(arg(args, "from")?))?;
            let to = self.snapshot(project, Some(arg(args, "to")?))?;
            return Ok(diff(&from, &to));
        }
        let s = self.snapshot(
            project,
            snapshot_id.or_else(|| args.get("snapshot").and_then(Value::as_str)),
        )?;
        match operation {
            "status" => {
                let mut stmt = self.db.prepare(
                    "SELECT p.snapshot FROM project_snapshots p
                     JOIN events e ON e.project=p.project AND e.snapshot=p.snapshot
                     WHERE p.project=?1 GROUP BY p.snapshot ORDER BY max(e.sequence) DESC",
                )?;
                let ids = stmt
                    .query_map([project], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                let mut snapshots = Vec::new();
                for id in ids {
                    let item = self.snapshot(project, Some(&id))?;
                    snapshots.push(json!({"id":id,"revision":item.source.git.revision,"dirty":item.source.git.dirty,
                        "files":item.source.files.len(),"objects":item.objects.len(),
                        "relationships":item.relationships.len(),"diagnostics":item.diagnostics.len()}));
                }
                Ok(
                    json!({"project":project_record,"snapshots":snapshots,"snapshot_id":s.id,
                    "coverage":coverage(&s),"diagnostics":s.diagnostics,
                    "freshness":crate::freshness::freshness(Path::new(&project_record.root), &s.source),
                    "limitations":["Static observations only. Dynamic dispatch, macros and external package resolution are not evaluated."]}),
                )
            }
            "search" => {
                let query = arg(args, "query")?;
                ensure!(query.len() <= 4096, "query too long");
                let limit = args
                    .get("limit")
                    .and_then(Value::as_u64)
                    .unwrap_or(20)
                    .clamp(1, 50) as usize;
                let terms: Vec<_> = query
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .filter(|s| !s.is_empty())
                    .take(32)
                    .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
                    .collect();
                ensure!(!terms.is_empty(), "query needs a word or symbol");
                let fts = terms.join(" AND ");
                let count:usize=self.db.query_row(
                    "SELECT count(*) FROM object_search WHERE snapshot=?1 AND object_search MATCH ?2",
                    params![s.id,fts],|r|r.get(0))?;
                let mut stmt = self.db.prepare(
                    "SELECT id FROM object_search WHERE snapshot=?1 AND object_search MATCH ?2
                     ORDER BY bm25(object_search,0,0,5,2,1),path,id LIMIT ?3",
                )?;
                let ids = stmt
                    .query_map(params![s.id, fts, limit], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                let mut results = Vec::new();
                for id in ids {
                    let o = s
                        .objects
                        .iter()
                        .find(|o| o.id == id)
                        .context("search index stale; rebuild indexes")?;
                    results.push(json!({"object":o,"evidence":evidence(&s,o)?}));
                }
                Ok(json!({"snapshot_id":s.id,"results":results,"total":count,"limit":limit}))
            }
            "get" | "evidence" => {
                let id = arg(args, "id")?;
                let o = s
                    .objects
                    .iter()
                    .find(|o| o.id == id)
                    .context("object not found in this snapshot")?;
                let ev = evidence(&s, o)?;
                if operation == "evidence" {
                    return Ok(json!({"snapshot_id":s.id,"evidence":ev}));
                }
                let relations: Vec<_> = s
                    .relationships
                    .iter()
                    .filter(|r| r.from == id || r.to.as_deref() == Some(id))
                    .collect();
                Ok(json!({"snapshot_id":s.id,"object":o,"evidence":ev,"relationships":relations}))
            }
            "dependencies" => dependencies(&self.db, &s, args),
            "verify" => {
                validate_snapshot(&s)?;
                Ok(
                    json!({"snapshot_id":s.id,"valid":true,"files":s.source.files.len(),"objects":s.objects.len()}),
                )
            }
            _ => bail!("unknown read operation"),
        }
    }
}

fn arg<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .with_context(|| format!("missing {key}"))
}

fn rebuild(db: &Connection, s: &Snapshot) -> Result<()> {
    db.execute("DELETE FROM object_search WHERE snapshot=?1", [&s.id])?;
    db.execute("DELETE FROM relationship_index WHERE snapshot=?1", [&s.id])?;
    for (ordinal, relationship) in s.relationships.iter().enumerate() {
        db.execute(
            "INSERT INTO relationship_index(snapshot,ordinal,from_id,to_id) VALUES(?1,?2,?3,?4)",
            params![s.id, ordinal, relationship.from, relationship.to],
        )?;
    }
    let files: BTreeMap<_, _> = s.source.files.iter().map(|f| (&f.path, f)).collect();
    for o in &s.objects {
        let file = files.get(&o.path).context("object has no source")?;
        let body = file
            .content
            .get(o.start_byte..o.end_byte)
            .context("object has invalid span")?;
        db.execute(
            "INSERT INTO object_search(snapshot,id,name,path,body) VALUES(?1,?2,?3,?4,?5)",
            params![s.id, o.id, o.name, o.path, body],
        )?;
    }
    Ok(())
}

pub fn coverage(s: &Snapshot) -> Value {
    let mut languages: BTreeMap<String, usize> = BTreeMap::new();
    for f in &s.source.files {
        *languages
            .entry(parser::language(&f.path).into())
            .or_default() += 1;
    }
    json!({"files":s.source.files.len(),"objects":s.objects.len(),"relationships":s.relationships.len(),
        "resolved":s.relationships.iter().filter(|r|r.resolution=="resolved").count(),
        "unresolved":s.relationships.iter().filter(|r|r.resolution!="resolved").count(),
        "languages":languages,
        "partial":s.diagnostics.iter().any(|d|matches!(d.code.as_str(),"parse_error"|"extraction_limit")),
        "excluded":s.source.diagnostics.iter().filter(|d| matches!(d.code.as_str(),
            "excluded"|"symlink"|"ignored"|"special_file"|"oversized"|"binary"|"credential_detected")).count()})
}

fn dependencies(db: &Connection, s: &Snapshot, args: &Value) -> Result<Value> {
    let id = arg(args, "id")?;
    let root = s
        .objects
        .iter()
        .find(|o| o.id == id)
        .context("object not found")?;
    let depth = args
        .get("depth")
        .and_then(Value::as_u64)
        .unwrap_or(2)
        .clamp(1, 8);
    let direction = args
        .get("direction")
        .and_then(Value::as_str)
        .unwrap_or("both");
    ensure!(
        matches!(direction, "incoming" | "outgoing" | "both"),
        "invalid dependency direction"
    );
    let mut visited = BTreeSet::from([id.to_string()]);
    let mut front = visited.clone();
    let mut relationships = BTreeSet::new();
    let mut truncated = false;
    let count: usize = db.query_row(
        "SELECT count(*) FROM relationship_index WHERE snapshot=?1",
        [&s.id],
        |r| r.get(0),
    )?;
    ensure!(
        count == s.relationships.len(),
        "relationship index is incomplete; rebuild indexes or recompile with --full"
    );
    for _ in 0..depth {
        let mut next = BTreeSet::new();
        'frontier: for vertex in &front {
            let mut statement = db.prepare(
                "SELECT ordinal,from_id,to_id FROM relationship_index
                 WHERE snapshot=?1 AND from_id=?2 AND ?3!='incoming'
                 UNION
                 SELECT ordinal,from_id,to_id FROM relationship_index
                 WHERE snapshot=?1 AND to_id=?2 AND ?3!='outgoing'
                 ORDER BY ordinal LIMIT 1001",
            )?;
            let candidates = statement.query_map(params![s.id, vertex, direction], |row| {
                Ok((
                    row.get::<_, usize>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?;
            for candidate in candidates {
                let (ordinal, from, to) = candidate?;
                let r = s
                    .relationships
                    .get(ordinal)
                    .context("relationship index has invalid ordinal")?;
                ensure!(
                    r.from == from && r.to == to,
                    "relationship index differs from captured facts; rebuild indexes"
                );
                let outgoing = direction != "incoming" && front.contains(&r.from);
                let incoming =
                    direction != "outgoing" && r.to.as_ref().is_some_and(|v| front.contains(v));
                if relationships.contains(r) {
                    continue;
                }
                if relationships.len() >= 1000 {
                    truncated = true;
                    break 'frontier;
                }
                relationships.insert(r.clone());
                if outgoing {
                    if let Some(to) = &r.to {
                        next.insert(to.clone());
                    }
                }
                if incoming {
                    next.insert(r.from.clone());
                }
            }
        }
        next.retain(|v| !visited.contains(v));
        visited.extend(next.iter().cloned());
        front = next;
        if front.is_empty() || truncated {
            break;
        }
    }
    let objects: Vec<_> = s
        .objects
        .iter()
        .filter(|o| visited.contains(&o.id))
        .collect();
    let unresolved: Vec<_> = relationships
        .iter()
        .filter(|r| r.resolution != "resolved")
        .cloned()
        .collect();
    Ok(
        json!({"snapshot_id":s.id,"root":root,"objects":objects,"relationships":relationships,
        "unresolved":unresolved,"truncated":truncated,
        "limitations":["Dependency observations are not guaranteed breakage predictions.",
            "Candidate calls can be shadowed at runtime; unresolved relationships remain visible."]}),
    )
}

fn diff(a: &Snapshot, b: &Snapshot) -> Value {
    let left: BTreeMap<_, _> = a
        .source
        .files
        .iter()
        .map(|f| (&f.path, sha256(f.content.as_bytes())))
        .collect();
    let right: BTreeMap<_, _> = b
        .source
        .files
        .iter()
        .map(|f| (&f.path, sha256(f.content.as_bytes())))
        .collect();
    let added: Vec<_> = right.keys().filter(|p| !left.contains_key(*p)).collect();
    let removed: Vec<_> = left.keys().filter(|p| !right.contains_key(*p)).collect();
    let modified: Vec<_> = left
        .iter()
        .filter(|(p, h)| right.get(*p).is_some_and(|r| r != *h))
        .map(|(p, _)| p)
        .collect();
    let old: BTreeSet<_> = a.objects.iter().map(|o| &o.id).collect();
    let new: BTreeSet<_> = b.objects.iter().map(|o| &o.id).collect();
    let objects_added: Vec<_> = b.objects.iter().filter(|o| !old.contains(&o.id)).collect();
    let objects_removed: Vec<_> = a.objects.iter().filter(|o| !new.contains(&o.id)).collect();
    json!({"from":a.id,"to":b.id,"added":added,"removed":removed,"modified":modified,
        "objects_added":objects_added,"objects_removed":objects_removed})
}
