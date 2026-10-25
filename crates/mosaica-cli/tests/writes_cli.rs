//! The write verbs against a served bundle. What they declare, insert, publish, delete and
//! suppress is what the server then serves, and still serves after a restart.

mod common;

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Output, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use arrow::array::{Array, ArrayRef, Float64Array, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use serde_json::Value;
use tempfile::TempDir;

use common::{deployment_with_control, mosaica, Bound, Ports, Server, OPERATOR_CREDENTIAL};

const OPERATOR: (&str, &str) = ("MOSAICA_CREDENTIAL", OPERATOR_CREDENTIAL);

/// A declaration of a plain view, a view group, a closed vocabulary whose values are read from a
/// file, a category attribute drawing on it, and a layer over the view, for a service whose bundle
/// holds none of them.
const MORE: &str = r#"
[sources]
points = "points.parquet"
roster = "roster.parquet"
moods  = "moods.parquet"

[defaults]
source = "points"

[[view]]
name             = "plane"
extent           = { x = [0.0, 1000.0], y = [0.0, 1000.0] }
point_visibility = { default = "public" }

[[view_group]]
name             = "years"
projection       = "none"
source           = "points"
fields           = { view = "year" }
extent           = { x = [0.0, 100.0], y = [0.0, 100.0] }
point_visibility = { default = "public" }
metadata         = { label = "text" }

[view_group.views]
source = "roster"
fields = { key = "year" }

[[vocabulary]]
name       = "mood"
width      = "u16"
value_set  = "closed"
visibility = "public"
source     = "moods"

[[attribute]]
name       = "mood"
type       = "category"
vocabulary = "mood"
index      = true

[[layer]]
name                      = "cli/flat"
views                     = ["plane"]
membership                = "enumerated"
hierarchy                 = { kind = "flat" }
visibility                = "public"
artifact_visibility       = { default = "inherited" }
require_member_visibility = "none"
"#;

/// A served deployment of the generated corpus: one view, `s0`, of 2,000 items whose unique `id`
/// is 0 to 1,999.
struct Served {
    dir: TempDir,
    server: Server,
    bound: Bound,
}

/// Serve the corpus with its control plane on a Unix socket or, without one, over HTTP, and with
/// `ingest` added to `mosaica.toml`.
fn serve(unix: bool, ingest: &str) -> Served {
    let dir = TempDir::new().unwrap();
    let control = match unix {
        true => format!("unix:{}", dir.path().join("control.sock").display()),
        false => "127.0.0.1:0".to_string(),
    };
    deployment_with_control(dir.path(), &Ports::chosen(), &control);
    let deployment = dir.path().join("mosaica.toml");
    let written = std::fs::read_to_string(&deployment).unwrap();
    std::fs::write(&deployment, format!("{written}\n{ingest}")).unwrap();
    let (server, bound) = Server::announced(dir.path());
    Served { dir, server, bound }
}

/// Stop the server and start it again over the same bundle and write-ahead log.
fn restart(served: Served) -> Served {
    let Served { dir, server, .. } = served;
    drop(server);
    let (server, bound) = Server::announced(dir.path());
    Served { dir, server, bound }
}

/// Runs `mosaica` in `dir` with `args`, the environment given and no other credential, and
/// `stdin` on its standard input.
fn run(dir: &Path, args: &[&str], env: &[(&str, &str)], stdin: &[u8]) -> Output {
    let mut child = mosaica()
        .current_dir(dir)
        .args(args)
        .env_remove("MOSAICA_CREDENTIAL")
        .env_remove("MOSAICA_TOKEN")
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // A command that does not read its standard input closes it; that is not this test's failure.
    let _ = child.stdin.take().unwrap().write_all(stdin);
    child.wait_with_output().unwrap()
}

/// The JSON a command that succeeded printed.
fn answer(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// The JSON a command that failed printed, for one that prints a report.
fn refused(output: Output) -> Value {
    assert!(!output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

/// A session reading every item.
fn token(served: &Served) -> String {
    let output = run(
        served.dir.path(),
        &[
            "session",
            "authorise",
            "--session",
            &served.bound.session,
            "--read-all",
        ],
        &[("MOSAICA_API_KEY", OPERATOR_CREDENTIAL)],
        b"",
    );
    answer(output)["token"].as_str().unwrap().to_owned()
}

/// `mosaica items` of the items in `view` whose `id` is one of `ids`, with `fields`.
fn items(served: &Served, token: &str, view: &str, fields: &str, ids: &[u64]) -> Vec<RecordBatch> {
    let out = served.dir.path().join("items.arrows");
    let filters = serde_json::json!({ "id": { "in": ids } }).to_string();
    let output = run(
        served.dir.path(),
        &[
            "items",
            "--server",
            &served.bound.viewer,
            "--token",
            token,
            "--view",
            view,
            "--fields",
            fields,
            "--filters",
            &filters,
            "--out",
            out.to_str().unwrap(),
        ],
        &[],
        b"",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    read_ipc(&out)
}

/// The `id`s of `ids` served in `view`, in order.
fn served_ids(served: &Served, token: &str, view: &str, ids: &[u64]) -> Vec<u64> {
    let mut found: Vec<u64> = u64s(&items(served, token, view, "id", ids), "id")
        .into_iter()
        .flatten()
        .collect();
    found.sort_unstable();
    found
}

fn read_ipc(path: &Path) -> Vec<RecordBatch> {
    arrow::ipc::reader::StreamReader::try_new(std::fs::File::open(path).unwrap(), None)
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn read_parquet(path: &Path) -> Vec<RecordBatch> {
    parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(
        std::fs::File::open(path).unwrap(),
    )
    .unwrap()
    .build()
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

/// A column of every batch, as unsigned integers.
fn u64s(batches: &[RecordBatch], name: &str) -> Vec<Option<u64>> {
    batches
        .iter()
        .flat_map(|batch| {
            let column =
                arrow::compute::cast(batch.column_by_name(name).unwrap(), &DataType::UInt64)
                    .unwrap();
            let column = column
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
                .clone();
            column.iter().collect::<Vec<_>>()
        })
        .collect()
}

/// A column of every batch, as text.
fn texts(batches: &[RecordBatch], name: &str) -> Vec<Option<String>> {
    batches
        .iter()
        .flat_map(|batch| {
            let column =
                arrow::compute::cast(batch.column_by_name(name).unwrap(), &DataType::Utf8).unwrap();
            let column = column
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .clone();
            column
                .iter()
                .map(|cell| cell.map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn write_parquet(path: &Path, columns: Vec<(&str, ArrayRef)>) {
    let schema = Arc::new(Schema::new(
        columns
            .iter()
            .map(|(name, column)| Field::new(*name, column.data_type().clone(), true))
            .collect::<Vec<_>>(),
    ));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        columns.into_iter().map(|(_, c)| c).collect(),
    )
    .unwrap();
    let mut writer =
        parquet::arrow::ArrowWriter::try_new(std::fs::File::create(path).unwrap(), schema, None)
            .unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();
}

/// `/v1/meta` as `token` reads it.
fn meta(served: &Served, token: &str) -> Value {
    let response = reqwest::blocking::Client::new()
        .get(format!("{}/v1/meta", served.bound.viewer))
        .bearer_auth(token)
        .send()
        .unwrap();
    assert!(
        response.status().is_success(),
        "/v1/meta: {}",
        response.status()
    );
    serde_json::from_slice(&response.bytes().unwrap()).unwrap()
}

/// The `key` of every object in `meta[list]`.
fn named(meta: &Value, list: &str, key: &str) -> BTreeSet<String> {
    meta[list]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry[key].as_str().map(str::to_owned))
        .collect()
}

/// What the first test wrote, as served: the view, the group's view, the attribute and the layer
/// are there; the rows are in the view with their moods; the layer's artifact counts its members.
fn check_written(served: &Served, ids_file: &Path) {
    let token = token(served);
    let meta = meta(served, &token);
    let views = named(&meta, "views", "id");
    assert!(
        views.contains("plane") && views.contains("years:2020"),
        "{meta}"
    );
    assert!(
        named(&meta, "declared_scalars", "name").contains("mood"),
        "{meta}"
    );
    assert!(
        named(&meta, "layers", "name").contains("cli/flat"),
        "{meta}"
    );

    let wanted: Vec<u64> = (0..10).chain(5000..5005).collect();
    let batches = items(served, &token, "plane", "id,mood", &wanted);
    let ids = u64s(&batches, "id");
    let moods = texts(&batches, "mood");
    let mut found: Vec<u64> = ids.iter().flatten().copied().collect();
    found.sort_unstable();
    assert_eq!(found, wanted);
    for (id, mood) in ids.iter().zip(&moods) {
        let id = id.unwrap();
        let expected = match id {
            id if id >= 5000 => None,
            id if id % 2 == 0 => Some("calm".to_string()),
            _ => Some("glad".to_string()),
        };
        assert_eq!(mood, &expected, "item {id}");
    }

    // The ids file names the items the insert created, by the `mosaica_id` the server serves.
    let created = items(
        served,
        &token,
        "plane",
        "id",
        &(5000..5005).collect::<Vec<_>>(),
    );
    let served_ids: BTreeSet<u64> = u64s(&created, "mosaica_id").into_iter().flatten().collect();
    let written = read_parquet(ids_file);
    assert_eq!(u64s(&written, "row"), (0..5).map(Some).collect::<Vec<_>>());
    let written: BTreeSet<u64> = u64s(&written, "mosaica_id").into_iter().flatten().collect();
    assert_eq!(written, served_ids);

    let out = served.dir.path().join("artifacts.arrows");
    let output = run(
        served.dir.path(),
        &[
            "artifacts",
            "--server",
            &served.bound.viewer,
            "--token",
            &token,
            "--view",
            "plane",
            "--layer",
            "cli/flat",
            "--fields",
            "key,masked_count",
            "--out",
            out.to_str().unwrap(),
        ],
        &[],
        b"",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let artifacts = read_ipc(&out);
    assert_eq!(texts(&artifacts, "key"), vec![Some("a".to_string())]);
    assert_eq!(u64s(&artifacts, "masked_count"), vec![Some(4)]);
}

#[test]
fn a_declaration_and_what_is_written_into_it_are_served_and_survive_a_restart() {
    let served = serve(false, "");
    let dir = served.dir.path().to_owned();
    let control = served.bound.control.clone();
    std::fs::write(dir.join("more.toml"), MORE).unwrap();
    write_parquet(
        &dir.join("moods.parquet"),
        vec![
            (
                "key",
                Arc::new(StringArray::from(vec!["calm", "glad"])) as ArrayRef,
            ),
            ("title", Arc::new(StringArray::from(vec!["Calm", "Glad"]))),
        ],
    );
    let write = |args: &[&str], stdin: &[u8]| {
        let mut all = args.to_vec();
        all.extend(["--control", control.as_str()]);
        run(&dir, &all, &[OPERATOR], stdin)
    };

    // Declared once, every request is taken; declared again, every request finds what it
    // declares already there.
    let first = answer(write(&["declare", "--config", "more.toml"], b""));
    assert_eq!(first["refused"], 0, "{first}");
    let again = answer(write(&["declare", "--config", "more.toml"], b""));
    let statuses: Vec<&Value> = again["requests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|request| &request["status"])
        .collect();
    assert!(statuses.iter().all(|status| **status == 200), "{again}");
    answer(write(
        &[
            "declare",
            "view",
            "years:2020",
            "--body",
            r#"{"metadata": {"label": "twenty"}}"#,
        ],
        b"",
    ));

    // Items the bundle holds, added to the new view from NDJSON on stdin.
    let lines: String = (0..10)
        .map(|id| format!("{{\"id\": {id}, \"x\": {}.0, \"y\": 5.0}}\n", id * 10))
        .collect();
    let added = answer(write(
        &[
            "insert", "-", "--format", "ndjson", "--view", "plane", "--wait",
        ],
        lines.as_bytes(),
    ));
    assert_eq!(added["added"], 10, "{added}");
    assert_eq!(added["refused"], serde_json::json!([]), "{added}");

    // Their moods, from Parquet, into no view: the rows carry no coordinates.
    write_parquet(
        &dir.join("moods_of.parquet"),
        vec![
            (
                "id",
                Arc::new(UInt64Array::from_iter_values(0..10)) as ArrayRef,
            ),
            (
                "mood",
                Arc::new(StringArray::from_iter_values((0..10).map(|id| {
                    if id % 2 == 0 {
                        "calm"
                    } else {
                        "glad"
                    }
                }))),
            ),
        ],
    );
    let edited = answer(write(&["insert", "moods_of.parquet", "--wait"], b""));
    assert_eq!(edited["edited"], 10, "{edited}");

    // New items, with the ids the server gave them written out.
    write_parquet(
        &dir.join("new.parquet"),
        vec![
            (
                "id",
                Arc::new(UInt64Array::from_iter_values(5000..5005)) as ArrayRef,
            ),
            (
                "x",
                Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0, 4.0, 5.0])),
            ),
            ("y", Arc::new(Float64Array::from(vec![9.0; 5]))),
        ],
    );
    let ids_file = dir.join("ids.parquet");
    let created = answer(write(
        &[
            "insert",
            "new.parquet",
            "--view",
            "plane",
            "--wait",
            "--ids-out",
            ids_file.to_str().unwrap(),
        ],
        b"",
    ));
    assert_eq!(created["created"], 5, "{created}");

    // An artifact of three members, grown by a fourth.
    answer(write(
        &[
            "publish",
            "cli/flat",
            "--body",
            r#"{"artifacts": [{"key": "a", "members": {"id": [0, 1, 2]}}]}"#,
        ],
        b"",
    ));
    answer(write(
        &["grow", "cli/flat", "--body-file", "-"],
        br#"{"artifacts": [{"key": "a", "members": {"id": [3]}}]}"#,
    ));
    let flushed = answer(write(&["flush", "--wait"], b""));
    assert_eq!(flushed["visible"], true, "{flushed}");

    check_written(&served, &ids_file);
    let served = restart(served);
    check_written(&served, &ids_file);

    // Dropped, the layer and the group's view are served no more, then or after a restart.
    let control = served.bound.control.clone();
    let dropped = |args: &[&str]| {
        let mut all = args.to_vec();
        all.extend(["--control", control.as_str(), "--wait"]);
        answer(run(served.dir.path(), &all, &[OPERATOR], b""))
    };
    dropped(&["drop", "layer", "cli/flat"]);
    dropped(&["drop", "view", "years:2020"]);
    let gone = |served: &Served| {
        let meta = meta(served, &token(served));
        assert!(
            !named(&meta, "layers", "name").contains("cli/flat"),
            "{meta}"
        );
        assert!(
            !named(&meta, "views", "id").contains("years:2020"),
            "{meta}"
        );
        assert!(named(&meta, "views", "id").contains("plane"), "{meta}");
    };
    gone(&served);
    let served = restart(served);
    gone(&served);
}

#[test]
fn a_file_of_many_pages_lands_each_row_once() {
    let served = serve(
        true,
        "[ingest]\ningest_max_batch_rows = 7\ningest_max_batch_bytes = 4096\n",
    );
    let dir = served.dir.path();
    let control = served.bound.control.as_str();

    // Fifty rows, three of them long enough that a page holding them is over the byte cap.
    write_parquet(
        &dir.join("rows.parquet"),
        vec![
            (
                "id",
                Arc::new(UInt64Array::from_iter_values(6000..6050)) as ArrayRef,
            ),
            (
                "x",
                Arc::new(Float64Array::from_iter_values((0..50).map(f64::from))),
            ),
            ("y", Arc::new(Float64Array::from(vec![7.0; 50]))),
            (
                "tag",
                Arc::new(StringArray::from_iter_values((0..50).map(
                    |row| match row {
                        20..=22 => "x".repeat(1500),
                        row => format!("t{row}"),
                    },
                ))),
            ),
        ],
    );
    let report = answer(run(
        dir,
        &[
            "insert",
            "rows.parquet",
            "--view",
            "s0",
            "--wait",
            "--control",
            control,
        ],
        &[OPERATOR],
        b"",
    ));
    assert_eq!(report["rows"], 50, "{report}");
    assert_eq!(report["created"], 50, "{report}");
    assert_eq!(report["refused_pages"], serde_json::json!([]), "{report}");
    // Eight pages of seven rows at most, and more where the byte cap split one.
    assert!(report["pages"].as_u64().unwrap() > 8, "{report}");

    // A writer that may not read the status report writes with the figures it states, or with
    // the server's documented ones.
    let admin = |args: &[&str]| {
        let mut all = args.to_vec();
        all.extend(["--control", control]);
        answer(run(dir, &all, &[OPERATOR], b""))
    };
    admin(&["principal", "create", "writer", "--kind", "service"]);
    admin(&["grant", "--principal", "writer", "--permission", "write"]);
    admin(&[
        "grant",
        "--principal",
        "writer",
        "--permission",
        "write-all",
    ]);
    let key = admin(&["key", "create", "writer"])["key"]
        .as_str()
        .unwrap()
        .to_owned();
    let writer = [("MOSAICA_CREDENTIAL", key.as_str())];
    let rows = |first: u64| -> String {
        (first..first + 5)
            .map(|id| format!("{{\"id\": {id}, \"x\": 1.0, \"y\": 2.0}}\n"))
            .collect()
    };
    let documented = answer(run(
        dir,
        &[
            "insert",
            "-",
            "--format",
            "ndjson",
            "--view",
            "s0",
            "--wait",
            "--control",
            control,
        ],
        &writer,
        rows(7000).as_bytes(),
    ));
    assert_eq!(documented["created"], 5, "{documented}");
    let stated = answer(run(
        dir,
        &[
            "insert",
            "-",
            "--format",
            "ndjson",
            "--view",
            "s0",
            "--wait",
            "--page-rows",
            "2",
            "--page-bytes",
            "4096",
            "--control",
            control,
        ],
        &writer,
        rows(7100).as_bytes(),
    ));
    assert_eq!(stated["pages"], 3, "{stated}");
    assert_eq!(stated["created"], 5, "{stated}");

    let token = token(&served);
    let wanted: Vec<u64> = (6000..6050).chain(7000..7005).chain(7100..7105).collect();
    assert_eq!(served_ids(&served, &token, "s0", &wanted), wanted);
}

#[test]
fn deletions_and_suppressions_hold_after_a_restart() {
    let served = serve(false, "");
    let dir = served.dir.path().to_owned();
    let control = served.bound.control.clone();
    let write = |args: &[&str], stdin: &[u8]| {
        let mut all = args.to_vec();
        all.extend(["--control", control.as_str()]);
        run(&dir, &all, &[OPERATOR], stdin)
    };
    let token = token(&served);
    let first: Vec<u64> = (0..7).collect();
    let batches = items(&served, &token, "s0", "id", &first);
    let mosaica_id = |id: u64| {
        u64s(&batches, "id")
            .into_iter()
            .zip(u64s(&batches, "mosaica_id"))
            .find(|(held, _)| *held == Some(id))
            .and_then(|(_, mosaica_id)| mosaica_id)
            .unwrap()
            .to_string()
    };

    let deleted = answer(write(
        &[
            "delete",
            "--id",
            &mosaica_id(0),
            "--match",
            r#"{"id": "1"}"#,
        ],
        b"",
    ));
    assert_eq!(deleted["accepted"], 2, "{deleted}");
    let deleted = answer(write(
        &["delete", "--file", "-", "--format", "ndjson"],
        b"{\"id\": 2}\n",
    ));
    assert_eq!(deleted["accepted"], 1, "{deleted}");
    write_parquet(
        &dir.join("hide.parquet"),
        vec![(
            "id",
            Arc::new(UInt64Array::from_iter_values([3, 4])) as ArrayRef,
        )],
    );
    let hidden = answer(write(&["suppress", "--file", "hide.parquet"], b""));
    assert_eq!(hidden["accepted"], 2, "{hidden}");
    let shown = answer(write(&["unsuppress", "--id", &mosaica_id(4)], b""));
    assert_eq!(shown["accepted"], 1, "{shown}");

    // A change naming nothing is refused alone; under `--strict` it refuses its request.
    let nothing = answer(write(&["delete", "--match", r#"{"id": "999999"}"#], b""));
    assert_eq!(nothing["accepted"], 0, "{nothing}");
    assert_eq!(
        nothing["refused"][0]["reason"], "names_no_item",
        "{nothing}"
    );
    let strict = refused(write(
        &["delete", "--strict", "--match", r#"{"id": "999999"}"#],
        b"",
    ));
    assert_eq!(
        strict["refused_pages"].as_array().unwrap().len(),
        1,
        "{strict}"
    );

    assert_eq!(served_ids(&served, &token, "s0", &first), vec![4, 5, 6]);

    // A compaction removes the deleted rows; the items stay as they were.
    let request = answer(write(&["compact"], b""))["request"]
        .as_u64()
        .unwrap();
    let started = Instant::now();
    loop {
        let status = answer(write(&["status"], b""));
        if status["compaction"]["requests"]["answered"].as_u64() >= Some(request) {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(120), "{status}");
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(served_ids(&served, &token, "s0", &first), vec![4, 5, 6]);

    let served = restart(served);
    let token = self::token(&served);
    assert_eq!(served_ids(&served, &token, "s0", &first), vec![4, 5, 6]);

    // Without a credential nothing is sent; a view the service lacks refuses the page.
    let control = served.bound.control.clone();
    let bare = run(&dir, &["status", "--control", &control], &[], b"");
    assert!(!bare.status.success() && bare.stdout.is_empty(), "{bare:?}");
    let nowhere = refused(run(
        &dir,
        &[
            "insert",
            "-",
            "--format",
            "ndjson",
            "--view",
            "nowhere",
            "--control",
            &control,
        ],
        &[OPERATOR],
        b"{\"id\": 9000, \"x\": 1.0, \"y\": 1.0}\n",
    ));
    assert_eq!(nowhere["refused_pages"][0]["status"], 404, "{nowhere}");
}
