//! The write verbs: declaring and dropping, inserting rows, deleting and suppressing items,
//! publishing artifacts, flushing, compacting and the status report, each through the control
//! plane with the credential in `MOSAICA_CREDENTIAL`.
//!
//! A verb that sends one request prints the server's JSON answer on stdout, as the identity verbs
//! do. A verb that reads a file, `insert` and the change verbs, sends each page before reading the
//! next, then prints one JSON object totalling the answers on stdout and one line on stderr.

use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, AsArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Float64Type, Schema, TimeUnit};
use arrow::error::ArrowError;
use arrow::ipc::reader::{FileReader, StreamReader};
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;
use clap::{ArgGroup, Args, Subcommand, ValueEnum};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ProjectionMask;
use serde_json::{json, Map, Value};

use crate::http::{finish, segment, Answer, Control, Plane};
use crate::records::Output;

/// The counts an ingest answer gives, summed over the pages.
const INGEST_COUNTS: &[&str] = &[
    "created",
    "edited",
    "added",
    "unchanged",
    "joined",
    "minted",
    "clamped",
    "clipped",
    "over_bound",
];

/// The count a changes answer gives.
const CHANGES_COUNTS: &[&str] = &["accepted"];

/// The counts a vocabulary values answer gives.
const VALUES_COUNTS: &[&str] = &["added", "existing", "titles"];

/// How many values one page carries where `--page-rows` does not say. The route has a byte cap
/// and no row cap, so this is the client's own figure, the Python client's, and the byte cap
/// decides.
const VALUES_PER_PAGE: usize = 10_000;

const ARROW_STREAM: &str = "application/vnd.apache.arrow.stream";
const NDJSON: &str = "application/x-ndjson";
const JSON: &str = "application/json";

// ---------------------------------------------------------------------------------------------
// The arguments
// ---------------------------------------------------------------------------------------------

/// A request body: the route's JSON, given inline or read from a file.
#[derive(Args)]
#[group(required = true, multiple = false)]
pub(crate) struct BodyArg {
    /// The request body as JSON.
    #[arg(long, value_name = "JSON")]
    body: Option<String>,
    /// Read the request body from this file, or from stdin with `-`.
    #[arg(long, value_name = "PATH")]
    body_file: Option<PathBuf>,
}

impl BodyArg {
    fn value(&self) -> Result<Value, String> {
        let (text, from) = match (&self.body, &self.body_file) {
            (Some(text), _) => (text.clone(), "--body".to_string()),
            (None, Some(path)) if path == Path::new("-") => {
                let mut text = String::new();
                std::io::stdin()
                    .read_to_string(&mut text)
                    .map_err(|e| format!("reading the body from stdin: {e}"))?;
                (text, "stdin".to_string())
            }
            (None, Some(path)) => (
                std::fs::read_to_string(path)
                    .map_err(|e| format!("reading {}: {e}", path.display()))?,
                path.display().to_string(),
            ),
            (None, None) => return Err("give the body with --body or --body-file".into()),
        };
        serde_json::from_str(&text)
            .map_err(|e| format!("{from} is not JSON ({e}); write the route's request body"))
    }
}

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
pub(crate) struct DeclareArgs {
    #[command(subcommand)]
    command: Option<DeclareCommand>,
    /// The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket.
    #[arg(long, value_name = "ADDRESS", required = true)]
    control: Option<String>,
    /// Without `--config`, read this `mosaica.toml` instead of searching for one upward from the
    /// working directory.
    #[arg(long, value_name = "PATH", conflicts_with = "config")]
    deployment: Option<PathBuf>,
    /// Read this corpus declaration, and no `mosaica.toml`, instead of `[build] schema` in
    /// `mosaica.toml`.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Read the source NAME in the declaration's `[sources]` from PATH instead. Repeatable, and
    /// the same override `mosaica check` takes. Of the sources, only a vocabulary's values are
    /// read.
    #[arg(long = "file", value_name = "NAME=PATH", value_parser = crate::parse_file_binding)]
    file: Vec<(String, PathBuf)>,
}

#[derive(Subcommand)]
pub(crate) enum DeclareCommand {
    /// Declare an attribute, a column the items carry, named in the body.
    ///
    /// The body is the `[[attribute]]` block as JSON, as `mosaica check --payloads` prints it
    /// under `attributes`, such as `{"name": "year", "type": "i32", "index": true}`. An attribute
    /// declared on a running service cannot be rendered.
    Attribute {
        #[command(flatten)]
        control: Control,
        #[command(flatten)]
        body: BodyArg,
    },
    /// Declare a vocabulary, the values a category attribute draws on.
    ///
    /// The body is the `[[vocabulary]]` block as JSON without its name, as `mosaica check
    /// --payloads` prints it under `vocabularies`. Its `values`, each a `key` and an optional
    /// `title`, are added as `mosaica insert --vocabulary` adds them, and the server draws every
    /// code.
    Vocabulary {
        #[command(flatten)]
        control: Control,
        /// The vocabulary's name.
        name: String,
        #[command(flatten)]
        body: BodyArg,
    },
    /// Declare a view, or add a view to a view group.
    ///
    /// For a view, NAME is its name and the body is the `[[view]]` block as JSON without its
    /// name, as `mosaica check --payloads` prints it under `views`. For a view of a view group,
    /// NAME is `<group>:<key>` and the body is the view's record: its `metadata`, one value per
    /// name the group declares, and its `visibility` where it has one, such as `{"metadata":
    /// {"year": 2020}}`. The view takes the group's frame and projection, and its record cannot
    /// be changed once it exists.
    View {
        #[command(flatten)]
        control: Control,
        /// The view's name, or `<group>:<key>` for a view of a view group.
        name: String,
        #[command(flatten)]
        body: BodyArg,
    },
    /// Declare a view group, with no views. `mosaica declare view <group>:<key>` adds each one.
    ///
    /// The body is the `[[view_group]]` block as JSON without its name, as `mosaica check
    /// --payloads` prints it under `view_groups`.
    ViewGroup {
        #[command(flatten)]
        control: Control,
        /// The group's name.
        name: String,
        #[command(flatten)]
        body: BodyArg,
    },
    /// Declare an annotation layer, named in the body.
    ///
    /// The body is the `[[layer]]` block as JSON, as `mosaica check --payloads` prints it under
    /// `layers`. The answer carries the layer's `mosaica_id`.
    Layer {
        #[command(flatten)]
        control: Control,
        #[command(flatten)]
        body: BodyArg,
    },
    /// Set the shard size: the high water at which the shard taking new items is sealed and the
    /// next one opened. A size at or below the open shard's high water seals it at once.
    Shards {
        #[command(flatten)]
        control: Control,
        /// The size, a number of items above 0.
        #[arg(long, value_name = "ITEMS")]
        size: u64,
    },
}

/// Where rows are read from, besides the file itself.
#[derive(Args)]
pub(crate) struct Input {
    /// `parquet`, `ipc` for an Arrow IPC stream or file, or `ndjson` for one JSON object per line.
    /// Without it the format is the file's extension: `.parquet`, `.arrows` or `.arrow`, `.ndjson`
    /// or `.jsonl`. Stdin needs it, and takes `ipc` as a stream, or `ndjson`.
    #[arg(long, value_enum, requires = "file")]
    format: Option<InputFormat>,
    /// Send only these columns, comma-separated. Without it every column is sent. A name the file
    /// has no column for is refused; in NDJSON, a row without one is sent without it.
    #[arg(long, value_name = "NAMES", value_delimiter = ',', requires = "file")]
    columns: Option<Vec<String>>,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum InputFormat {
    Parquet,
    Ipc,
    Ndjson,
}

#[derive(Args)]
pub(crate) struct InsertArgs {
    #[command(flatten)]
    control: Control,
    /// The rows: a file, or `-` for stdin.
    file: PathBuf,
    /// The view the rows are for, sent as `x-mosaica-view`. Without it a deployment with one view
    /// takes the rows into that view, and one with several into no view, where they carry no
    /// coordinates and change only what the items store.
    #[arg(long, conflicts_with = "vocabulary")]
    view: Option<String>,
    /// Add the rows to this vocabulary's values instead, through `PATCH
    /// /control/vocabularies/{name}/values`. Each row has a `key` and may have a `title`. A key
    /// the vocabulary does not hold is added with a code the server draws, and a title replaces a
    /// held key's title. The report counts `added`, `existing` and `titles` in place of the rows'
    /// counts.
    #[arg(long, value_name = "NAME", conflicts_with_all = ["strict", "ids_out"])]
    vocabulary: Option<String>,
    #[command(flatten)]
    input: Input,
    /// Refuse a whole page at its first refused row, with `409`, instead of applying its other
    /// rows.
    #[arg(long)]
    strict: bool,
    /// The most rows in one request, above 0. Default: `limits.ingest.max_batch_rows` from
    /// `/control/status`, or 10,000 with `--vocabulary`; where the credential cannot read
    /// `/control/status`, the server's documented default, 10,000. The figures used are printed
    /// on stderr.
    #[arg(long, value_name = "N")]
    page_rows: Option<usize>,
    /// The most bytes in one request body, above 0, in bytes or with a `k`, `m` or `g` suffix. A
    /// page whose body is larger is split in half until each part fits. Default:
    /// `limits.ingest.max_batch_bytes` from `/control/status`, or
    /// `limits.declarations.max_body_bytes` with `--vocabulary`; where the credential cannot read
    /// `/control/status`, the server's documented default, 16 MiB, or 2 MiB with `--vocabulary`.
    #[arg(long, value_name = "SIZE", value_parser = crate::parse_byte_size)]
    page_bytes: Option<u64>,
    /// Once every page is sent, send `POST /control/flush?wait=visible`, and end once readers see
    /// the rows, or after the server's `serve.visible_wait_max_secs` with `visible` false.
    /// Without it the rows become visible at the server's next publication.
    #[arg(long)]
    wait: bool,
    /// Write each row's `mosaica_id` to this file, ending in `.arrows` or `.parquet`: the columns
    /// `row`, its position in the input from 0, and `mosaica_id`, null for a row that was refused
    /// or whose page was.
    #[arg(long, value_name = "PATH")]
    ids_out: Option<PathBuf>,
}

#[derive(Args)]
#[command(group(ArgGroup::new("named").required(true).multiple(true).args(["ids", "matches", "file"])))]
pub(crate) struct ChangeArgs {
    #[command(flatten)]
    control: Control,
    /// An item or an annotation by its `mosaica_id`. Repeatable.
    #[arg(long = "id", value_name = "MOSAICA_ID")]
    ids: Vec<u64>,
    /// An item by the values that name it, as a JSON object of its `mosaica_id` and unique
    /// attributes, such as `{"doi": "10.1/x"}`. Repeatable.
    #[arg(long = "match", value_name = "JSON")]
    matches: Vec<String>,
    /// A file whose every row names one item by its columns, `mosaica_id` and unique
    /// attributes, or `-` for stdin. A column that names no item is not read, and is listed in
    /// `ignored_columns`, as is a column of a type a change does not carry, such as a list, which
    /// is not sent.
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,
    #[command(flatten)]
    input: Input,
    /// Refuse a whole request at its first change naming no item, with `404`, or naming two,
    /// with `409`, instead of applying its other changes.
    #[arg(long)]
    strict: bool,
    /// Once every change is sent, send `POST /control/flush?wait=visible`, and end once readers
    /// see the publication. A deletion or suppression is in force from the moment it is accepted
    /// either way.
    #[arg(long)]
    wait: bool,
}

#[derive(Args)]
pub(crate) struct ArtifactArgs {
    #[command(flatten)]
    control: Control,
    /// The layer's name.
    layer: String,
    #[command(flatten)]
    body: BodyArg,
    /// Refuse the whole request at its first member naming no item, with `404`, or naming two,
    /// with `409`, instead of leaving the member out.
    #[arg(long)]
    strict: bool,
}

#[derive(Subcommand)]
pub(crate) enum DropCommand {
    /// Drop an annotation layer. It and its artifacts stop being served at once. Its name may be
    /// declared again, as a new layer that none of the dropped layer's `mosaica_id`s names.
    Layer {
        #[command(flatten)]
        control: Control,
        /// The layer's name.
        name: String,
        /// Answer only once readers no longer see the layer, or after the server's
        /// `serve.visible_wait_max_secs` with `visible` false.
        #[arg(long)]
        wait: bool,
    },
    /// Drop a view of a view group. The items it leaves in no view are deleted, and the answer's
    /// `deleted` counts them. Its key may be created again, as a view with none of the old one's
    /// rows.
    View {
        #[command(flatten)]
        control: Control,
        /// The view, as `<group>:<key>`.
        view: String,
        /// Answer only once readers no longer see the view, or after the server's
        /// `serve.visible_wait_max_secs` with `visible` false.
        #[arg(long)]
        wait: bool,
    },
}

#[derive(Args)]
pub(crate) struct FlushArgs {
    #[command(flatten)]
    control: Control,
    /// Answer only once readers see the publication, or after the server's
    /// `serve.visible_wait_max_secs` with `visible` false.
    #[arg(long)]
    wait: bool,
}

// ---------------------------------------------------------------------------------------------
// One request
// ---------------------------------------------------------------------------------------------

pub(crate) fn declare(args: DeclareArgs) -> ExitCode {
    let DeclareArgs {
        command,
        control,
        deployment,
        config,
        file,
    } = args;
    let Some(command) = command else {
        return declare_all(control, deployment, config, file);
    };
    let put = |control: &Control, path: &str, body: &BodyArg| {
        body.value()
            .and_then(|body| control.write("PUT", path, Some(body)))
    };
    let answer = match command {
        DeclareCommand::Attribute { control, body } => put(&control, "/control/attributes", &body),
        DeclareCommand::Vocabulary {
            control,
            name,
            body,
        } => put(
            &control,
            &format!("/control/vocabularies/{}", segment(&name)),
            &body,
        ),
        DeclareCommand::View {
            control,
            name,
            body,
        } => put(&control, &view_path(&name), &body),
        DeclareCommand::ViewGroup {
            control,
            name,
            body,
        } => put(
            &control,
            &format!("/control/view_groups/{}", segment(&name)),
            &body,
        ),
        DeclareCommand::Layer { control, body } => put(&control, "/control/layers", &body),
        DeclareCommand::Shards { control, size } => {
            control.write("PUT", "/control/shards", Some(json!({ "size": size })))
        }
    };
    finish("declare", answer)
}

pub(crate) fn artifacts(args: ArtifactArgs, method: &str, verb: &str) -> ExitCode {
    let path = format!(
        "/control/layers/{}/artifacts{}",
        segment(&args.layer),
        query(args.strict, false)
    );
    let answer = args
        .body
        .value()
        .and_then(|body| args.control.write(method, &path, Some(body)));
    finish(verb, answer)
}

pub(crate) fn drop_command(command: DropCommand) -> ExitCode {
    let answer = match command {
        DropCommand::Layer {
            control,
            name,
            wait,
        } => control.write(
            "DELETE",
            &format!("/control/layers/{}{}", segment(&name), query(false, wait)),
            None,
        ),
        DropCommand::View {
            control,
            view,
            wait,
        } => match view.split_once(mosaica_types::view::GROUP_SEPARATOR) {
            Some(_) => control.write(
                "DELETE",
                &format!("{}{}", view_path(&view), query(false, wait)),
                None,
            ),
            None => Err(format!(
                "`{view}` is not a view of a view group, and only those can be dropped; name one \
                 as `<group>:<key>`"
            )),
        },
    };
    finish("drop", answer)
}

pub(crate) fn flush(args: FlushArgs) -> ExitCode {
    let path = format!("/control/flush{}", query(false, args.wait));
    finish("flush", args.control.write("POST", &path, None))
}

pub(crate) fn compact(control: Control) -> ExitCode {
    finish("compact", control.write("POST", "/control/compact", None))
}

pub(crate) fn status(control: Control) -> ExitCode {
    finish("status", control.write("GET", "/control/status", None))
}

/// `?strict=true` and `?wait=visible`, where asked for; the routes' defaults are the reverse.
fn query(strict: bool, wait: bool) -> String {
    let parts: Vec<&str> = [(strict, "strict=true"), (wait, "wait=visible")]
        .into_iter()
        .filter_map(|(asked, part)| asked.then_some(part))
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

/// A view's route: `/control/views/{name}`, or `/control/views/{group}/{key}` for `group:key`.
fn view_path(view: &str) -> String {
    match view.split_once(mosaica_types::view::GROUP_SEPARATOR) {
        Some((group, key)) => format!("/control/views/{}/{}", segment(group), segment(key)),
        None => format!("/control/views/{}", segment(view)),
    }
}

// ---------------------------------------------------------------------------------------------
// A whole declaration
// ---------------------------------------------------------------------------------------------

/// Every block of a corpus declaration, as the bodies `mosaica check --payloads` prints, in the
/// order the routes need: a vocabulary before the group or the attribute naming it, a group before
/// the group sharing its views and before its own views, a view before the layer drawn on it.
fn declare_all(
    control: Option<String>,
    deployment: Option<PathBuf>,
    config: Option<PathBuf>,
    file: Vec<(String, PathBuf)>,
) -> ExitCode {
    let requests = (|| {
        let plane = Plane::writer(control.as_deref().unwrap_or_default())?;
        let config = match config {
            Some(path) => crate::parse_declaration(&path, file)?,
            None => crate::resolve_declaration(deployment.as_deref(), None, file)?.config,
        };
        Ok::<_, String>((plane, declaration_requests(&config)?))
    })();
    let (plane, requests) = match requests {
        Ok(found) => found,
        Err(e) => {
            eprintln!("mosaica declare: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut sent = Vec::with_capacity(requests.len());
    let mut refused = 0;
    for (method, path, body) in &requests {
        let request = format!("{method} {path}");
        match plane.send(method, path, &[], Some((JSON, body))) {
            Ok(answer) => {
                refused += usize::from(!answer.ok());
                sent.push(json!({
                    "request": request,
                    "status": answer.status,
                    "answer": detail(&answer),
                }));
            }
            Err(e) => {
                refused += 1;
                sent.push(json!({ "request": request, "status": null, "answer": e }));
                break;
            }
        }
    }
    let accepted = sent.len() - refused;
    println!(
        "{}",
        json!({ "requests": sent, "accepted": accepted, "refused": refused })
    );
    eprintln!(
        "mosaica declare: {} of {} requests sent, {accepted} accepted, {refused} refused",
        sent.len(),
        requests.len()
    );
    if refused == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// One request: its method, path and JSON body.
type Request = (&'static str, String, Vec<u8>);

/// The requests that declare `config`.
fn declaration_requests(config: &mosaica_build::config::Config) -> Result<Vec<Request>, String> {
    let payloads = mosaica_build::config::control_payloads(config);
    let list = |key: &str| payloads[key].as_array().cloned().unwrap_or_default();
    let name = |entry: &Value| entry["name"].as_str().unwrap_or_default().to_owned();
    let mut requests = Vec::new();
    let put = |requests: &mut Vec<_>, path: String, body: &Value| {
        requests.push(("PUT", path, body.to_string().into_bytes()));
    };
    if let Some(shards) = payloads.get("shards") {
        put(&mut requests, "/control/shards".into(), shards);
    }
    for entry in list("vocabularies") {
        let vocabulary = name(&entry);
        let path = format!("/control/vocabularies/{}", segment(&vocabulary));
        let mut body = entry["body"].clone();
        // A sourced vocabulary's values are rows rather than declaration, and the payload leaves
        // them out: the first page goes on the declaration, as an inline set's values do, and
        // the rest follow it before anything that may name them.
        let sourced = config.schema.vocabularies.get(&vocabulary);
        let Some(declared) = sourced.filter(|_| entry.get("values_source").is_some()) else {
            put(&mut requests, path, &body);
            continue;
        };
        let rows = mosaica_build::config::vocabulary_value_rows(declared);
        let cap = mosaica_server::control::DECLARATION_MAX_BODY_BYTES
            .saturating_sub(body.to_string().len());
        let mut pages = Vec::new();
        for (index, chunk) in rows.chunks(VALUES_PER_PAGE).enumerate() {
            let first = index * VALUES_PER_PAGE;
            for piece in halved(chunk.len(), cap, |at, many| {
                Ok(values_body(&chunk[at..at + many]))
            }) {
                let (offset, many, _) = piece?;
                pages.push(&rows[first + offset..first + offset + many]);
            }
        }
        let mut pages = pages.into_iter();
        if let Some(page) = pages.next() {
            body["values"] = page.to_vec().into();
        }
        put(&mut requests, path.clone(), &body);
        for page in pages {
            requests.push(("PATCH", format!("{path}/values"), values_body(page)));
        }
    }
    for entry in list("view_groups") {
        let path = format!("/control/view_groups/{}", segment(&name(&entry)));
        put(&mut requests, path, &entry["body"]);
    }
    for entry in list("views") {
        let path = format!("/control/views/{}", segment(&name(&entry)));
        put(&mut requests, path, &entry["body"]);
    }
    for entry in mosaica_build::config::group_view_payloads(config).map_err(|e| e.to_string())? {
        let view = mosaica_store::group_view_id(
            entry["group"].as_str().unwrap_or_default(),
            entry["key"].as_str().unwrap_or_default(),
        );
        put(&mut requests, view_path(&view), &entry["body"]);
    }
    for body in list("attributes") {
        put(&mut requests, "/control/attributes".into(), &body);
    }
    for body in list("layers") {
        put(&mut requests, "/control/layers".into(), &body);
    }
    Ok(requests)
}

fn values_body(rows: &[Value]) -> Vec<u8> {
    json!({ "values": rows }).to_string().into_bytes()
}

// ---------------------------------------------------------------------------------------------
// Paged writes
// ---------------------------------------------------------------------------------------------

pub(crate) fn insert(args: InsertArgs) -> ExitCode {
    let report = args
        .control
        .writer()
        .and_then(|plane| match &args.vocabulary {
            Some(name) => insert_values(&plane, &args, name),
            None => insert_rows(&plane, &args),
        });
    print_report("insert", report)
}

pub(crate) fn change(args: ChangeArgs, op: &'static str) -> ExitCode {
    let report = args
        .control
        .writer()
        .and_then(|plane| changes(&plane, &args, op));
    print_report(op, report)
}

fn insert_rows(plane: &Plane, args: &InsertArgs) -> Result<Report, String> {
    let limits = Limits::read(
        plane,
        "insert",
        (args.page_rows, args.page_bytes),
        ("ingest", Some("max_batch_rows"), "max_batch_bytes"),
        (
            mosaica_config::defaults::DEFAULT_INGEST_MAX_BATCH_ROWS,
            mosaica_config::defaults::DEFAULT_INGEST_MAX_BATCH_BYTES,
        ),
    )?;
    let mut ids = args.ids_out.clone().map(Ids::new).transpose()?;
    let rows = open_rows(&args.file, &args.input, limits.rows)?;
    let path = format!("/control/ingest{}", query(args.strict, false));
    // The file's name, for a server log to say which file a batch came from, reduced to what a
    // header may carry.
    let stem: String = match args.file.file_stem() {
        Some(stem) if args.file != Path::new("-") => stem
            .to_string_lossy()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            .collect(),
        _ => "stdin".to_owned(),
    };
    let mut report = Report::default();
    let read = paged(rows, limits.rows, |first, page| {
        for piece in halved(page.len(), limits.bytes, |o, n| page.encode(o, n)) {
            let (offset, count, body) = piece?;
            let start = first + offset;
            // A fresh id per request, reused only when the same bytes go again, after a `429` or
            // when no answer came.
            let batch = format!("{stem}-{start}-{:032x}", rand::random::<u128>());
            let mut headers = vec![("x-mosaica-batch-id", batch.as_str())];
            if let Some(view) = &args.view {
                headers.push(("x-mosaica-view", view.as_str()));
            }
            let answer =
                plane.send_batch("POST", &path, &headers, Some((page.content_type(), &body)));
            let accepted = report.page(start, count, INGEST_COUNTS, answer, None);
            if let Some(ids) = ids.as_mut() {
                ids.write(start, count, accepted.as_ref())?;
            }
            if report.stopped.is_some() {
                return Ok(false);
            }
        }
        Ok(true)
    });
    if let Err(e) = read {
        report.stopped.get_or_insert(e);
    }
    report.close(plane, args.wait);
    if let Some(Err(e)) = ids.map(Ids::finish) {
        report.failed = Some(e);
    }
    Ok(report)
}

fn insert_values(plane: &Plane, args: &InsertArgs, name: &str) -> Result<Report, String> {
    let limits = Limits::read(
        plane,
        "insert",
        (args.page_rows, args.page_bytes),
        ("declarations", None, "max_body_bytes"),
        (
            VALUES_PER_PAGE,
            mosaica_server::control::DECLARATION_MAX_BODY_BYTES,
        ),
    )?;
    let rows = open_rows(&args.file, &args.input, limits.rows)?;
    let path = format!("/control/vocabularies/{}/values", segment(name));
    let mut report = Report::default();
    let read = paged(rows, limits.rows, |first, page| {
        let objects = page.objects(first)?;
        report.ignore(&objects.1);
        report.send_objects(
            plane,
            ("PATCH", &path),
            first,
            &objects.0,
            limits.bytes,
            VALUES_COUNTS,
            &values_body,
            false,
        )
    });
    if let Err(e) = read {
        report.stopped.get_or_insert(e);
    }
    report.close(plane, args.wait);
    Ok(report)
}

fn changes(plane: &Plane, args: &ChangeArgs, op: &'static str) -> Result<Report, String> {
    let mut named: Vec<Value> = args
        .ids
        .iter()
        .map(|id| json!({ "mosaica_id": id.to_string() }))
        .collect();
    for text in &args.matches {
        match serde_json::from_str(text) {
            Ok(Value::Object(fields)) => named.push(Value::Object(fields)),
            _ => {
                return Err(format!(
                    "--match {text} is not a JSON object; write the values that name the item, \
                     such as '{{\"doi\": \"10.1/x\"}}'"
                ))
            }
        }
    }
    let rows = match &args.file {
        Some(file) => Some(open_rows(
            file,
            &args.input,
            mosaica_server::control::CHANGES_MAX_ITEMS,
        )?),
        None => None,
    };
    let limits = Limits::read(
        plane,
        op,
        (None, None),
        ("changes", Some("max_changes_per_request"), "max_body_bytes"),
        (
            mosaica_server::control::CHANGES_MAX_ITEMS,
            mosaica_server::control::CHANGES_MAX_BODY_BYTES,
        ),
    )?;
    let path = format!("/control/changes{}", query(args.strict, false));
    let wrap = |rows: &[Value]| {
        let items: Vec<Value> = rows
            .iter()
            .map(|named| json!({ "op": op, "match": named }))
            .collect();
        Value::Array(items).to_string().into_bytes()
    };
    let mut report = Report::default();
    let mut more = true;
    for (index, chunk) in named.chunks(limits.rows).enumerate() {
        more = report.send_objects(
            plane,
            ("POST", &path),
            index * limits.rows,
            chunk,
            limits.bytes,
            CHANGES_COUNTS,
            &wrap,
            true,
        )?;
        if !more {
            break;
        }
    }
    if let (true, Some(rows)) = (more, rows) {
        let offset = named.len();
        let read = paged(rows, limits.rows, |first, page| {
            let objects = page.objects(offset + first)?;
            report.ignore(&objects.1);
            report.send_objects(
                plane,
                ("POST", &path),
                offset + first,
                &objects.0,
                limits.bytes,
                CHANGES_COUNTS,
                &wrap,
                true,
            )
        });
        if let Err(e) = read {
            report.stopped.get_or_insert(e);
        }
    }
    report.close(plane, args.wait);
    Ok(report)
}

/// The page sizes a paged write uses.
struct Limits {
    rows: usize,
    bytes: usize,
}

impl Limits {
    /// The figures given; for each not given, the route's published limit from `/control/status`,
    /// or the default where the status cannot be read. Prints the figures used and where each
    /// came from.
    fn read(
        plane: &Plane,
        verb: &str,
        (rows, bytes): (Option<usize>, Option<u64>),
        (route, rows_key, bytes_key): (&str, Option<&str>, &str),
        (default_rows, default_bytes): (usize, usize),
    ) -> Result<Limits, String> {
        if rows == Some(0) || bytes == Some(0) {
            return Err("--page-rows and --page-bytes take a number above 0".into());
        }
        let mut figures = [
            (rows, "--page-rows".to_string()),
            (bytes.map(|b| b as usize), "--page-bytes".to_string()),
        ];
        if figures.iter().any(|(figure, _)| figure.is_none()) {
            let answer = plane.call("GET", "/control/status", None)?;
            let published = &answer.json()["limits"][route];
            let keys = [rows_key, Some(bytes_key)];
            for ((figure, from), key) in figures.iter_mut().zip(keys) {
                if figure.is_some() {
                    continue;
                }
                let found = key
                    .filter(|_| answer.ok())
                    .and_then(|key| published[key].as_u64());
                *from = match (key, found) {
                    (Some(key), Some(_)) => format!("limits.{route}.{key} in /control/status"),
                    (Some(key), None) if answer.ok() => format!(
                        "the server's documented default; /control/status has no \
                         limits.{route}.{key}"
                    ),
                    (Some(_), None) => format!(
                        "the server's documented default; /control/status answered {}",
                        answer.status
                    ),
                    (None, _) => "the CLI's own figure; the route has no row limit".to_string(),
                };
                *figure = found.map(|n| n as usize);
            }
            figures[0].0 = figures[0].0.or(Some(default_rows));
            figures[1].0 = figures[1].0.or(Some(default_bytes));
        }
        let [(Some(rows), rows_from), (Some(bytes), bytes_from)] = figures else {
            unreachable!("every figure is filled above");
        };
        eprintln!(
            "mosaica {verb}: pages of at most {rows} rows ({rows_from}) and {bytes} bytes \
             ({bytes_from})"
        );
        Ok(Limits { rows, bytes })
    }
}

/// What a paged write's answers came to.
#[derive(Default)]
struct Report {
    pages: u64,
    rows: u64,
    accepted_pages: u64,
    counts: BTreeMap<String, u64>,
    replayed: u64,
    refused: Vec<Value>,
    refused_pages: Vec<Value>,
    ignored_columns: Vec<String>,
    publication: Option<u64>,
    visible: Option<bool>,
    /// The closing flush, where it was refused or went unanswered.
    flush: Option<Value>,
    /// Where and why the write stopped before the end of its input.
    stopped: Option<String>,
    /// What failed here once the requests were sent, such as writing `--ids-out`.
    failed: Option<String>,
}

impl Report {
    /// Fold in one request's answer: `count` rows from row `first`. Returns the answer's body
    /// where the request was accepted. `named` is what each row of the request named, where the
    /// refused rows should say.
    fn page(
        &mut self,
        first: usize,
        count: usize,
        keys: &[&str],
        answer: Result<Answer, String>,
        named: Option<&[Value]>,
    ) -> Option<Value> {
        self.pages += 1;
        self.rows += count as u64;
        let answer = match answer {
            Ok(answer) if answer.ok() => answer,
            Ok(answer) => {
                self.refused_pages.push(json!({
                    "first_row": first, "rows": count, "status": answer.status,
                    "detail": detail(&answer),
                }));
                return None;
            }
            Err(e) => {
                self.refused_pages.push(json!({
                    "first_row": first, "rows": count, "status": null, "detail": e,
                }));
                self.stopped = Some(format!("row {first}, whose request went unanswered"));
                return None;
            }
        };
        self.accepted_pages += 1;
        let body = answer.json();
        if body["replayed"] == Value::Bool(true) {
            self.replayed += 1;
        }
        for key in keys {
            if let Some(n) = body[*key].as_u64() {
                *self.counts.entry((*key).to_owned()).or_default() += n;
            }
        }
        if let Some(publication) = body["publication"].as_u64() {
            self.publication = self.publication.max(Some(publication));
        }
        for refused in body["refused"].as_array().into_iter().flatten() {
            let row = refused["row"].as_u64().unwrap_or_default() as usize;
            let mut entry = json!({ "row": first + row, "reason": refused["reason"] });
            if let Some(item) = named.and_then(|named| named.get(row)) {
                entry["match"] = item.clone();
            }
            self.refused.push(entry);
        }
        let ignored: Vec<String> = body["ignored_columns"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|column| column.as_str().map(str::to_owned))
            .collect();
        self.ignore(&ignored);
        Some(body)
    }

    /// Name columns the requests left out among the ignored ones.
    fn ignore(&mut self, columns: &[String]) {
        for column in columns {
            if !self.ignored_columns.contains(column) {
                self.ignored_columns.push(column.clone());
            }
        }
    }

    /// Send rows of JSON from row `first`, as many requests as the byte cap takes, each body
    /// `wrap` of its rows. Returns whether to go on.
    #[allow(clippy::too_many_arguments)]
    fn send_objects(
        &mut self,
        plane: &Plane,
        (method, path): (&str, &str),
        first: usize,
        rows: &[Value],
        cap: usize,
        keys: &[&str],
        wrap: &dyn Fn(&[Value]) -> Vec<u8>,
        name_refused: bool,
    ) -> Result<bool, String> {
        for piece in halved(rows.len(), cap, |o, n| Ok(wrap(&rows[o..o + n]))) {
            let (offset, count, body) = piece?;
            let answer = plane.send(method, path, &[], Some((JSON, &body)));
            let named = name_refused.then(|| &rows[offset..offset + count]);
            self.page(first + offset, count, keys, answer, named);
            if self.stopped.is_some() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// The closing flush under `--wait`, where a request was accepted.
    fn close(&mut self, plane: &Plane, wait: bool) {
        if !wait || self.accepted_pages == 0 {
            return;
        }
        match plane.call("POST", "/control/flush?wait=visible", None) {
            Ok(answer) if answer.ok() => {
                let body = answer.json();
                self.publication = body["publication"].as_u64().or(self.publication);
                self.visible = body["visible"].as_bool();
            }
            Ok(answer) => {
                self.flush = Some(json!({ "status": answer.status, "detail": detail(&answer) }));
            }
            Err(e) => self.flush = Some(json!({ "status": null, "detail": e })),
        }
    }

    fn ok(&self) -> bool {
        self.refused_pages.is_empty()
            && self.flush.is_none()
            && self.stopped.is_none()
            && self.failed.is_none()
    }

    fn json(&self) -> Value {
        let mut out = Map::new();
        out.insert("pages".into(), self.pages.into());
        out.insert("rows".into(), self.rows.into());
        for (key, n) in &self.counts {
            out.insert(key.clone(), (*n).into());
        }
        out.insert("replayed".into(), self.replayed.into());
        out.insert("refused".into(), self.refused.clone().into());
        out.insert("refused_pages".into(), self.refused_pages.clone().into());
        out.insert(
            "ignored_columns".into(),
            self.ignored_columns.clone().into(),
        );
        out.insert("publication".into(), self.publication.into());
        if let Some(visible) = self.visible {
            out.insert("visible".into(), visible.into());
        }
        if let Some(flush) = &self.flush {
            out.insert("flush".into(), flush.clone());
        }
        if let Some(stopped) = &self.stopped {
            out.insert("stopped".into(), stopped.clone().into());
        }
        Value::Object(out)
    }

    fn line(&self) -> String {
        let mut line = format!("{} rows in {} requests", self.rows, self.pages);
        let counts: Vec<String> = self
            .counts
            .iter()
            .map(|(key, n)| format!("{n} {key}"))
            .collect();
        if !counts.is_empty() {
            line.push_str(&format!(": {}", counts.join(", ")));
        }
        let mut reasons: BTreeMap<&str, usize> = BTreeMap::new();
        for refused in &self.refused {
            *reasons
                .entry(refused["reason"].as_str().unwrap_or("unknown"))
                .or_default() += 1;
        }
        line.push_str(&format!("; {} rows refused", self.refused.len()));
        if !reasons.is_empty() {
            let reasons: Vec<String> = reasons.iter().map(|(r, n)| format!("{r} {n}")).collect();
            line.push_str(&format!(" ({})", reasons.join(", ")));
        }
        line.push_str(&format!("; {} requests refused", self.refused_pages.len()));
        if self.replayed > 0 {
            line.push_str(&format!("; {} requests replayed", self.replayed));
        }
        match (self.publication, self.visible) {
            (Some(p), Some(true)) => line.push_str(&format!("; visible at publication {p}")),
            (Some(p), Some(false)) => line.push_str(&format!("; publication {p} not yet visible")),
            (Some(p), None) => line.push_str(&format!("; visible from publication {p}")),
            (None, _) => {}
        }
        if self.flush.is_some() {
            line.push_str("; the closing flush was refused");
        }
        if let Some(stopped) = &self.stopped {
            line.push_str(&format!("; stopped at {stopped}"));
        }
        line
    }
}

fn print_report(verb: &str, report: Result<Report, String>) -> ExitCode {
    match report {
        Ok(report) => {
            println!("{}", report.json());
            eprintln!("mosaica {verb}: {}", report.line());
            if let Some(failed) = &report.failed {
                eprintln!("mosaica {verb}: {failed}");
            }
            if report.ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("mosaica {verb}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// An answer's body as JSON, or as text where it is not JSON.
fn detail(answer: &Answer) -> Value {
    match answer.json() {
        Value::Null => answer.text().into(),
        json => json,
    }
}

/// `(first, rows, body)` over `count` rows, every body at most `cap` bytes, each encoded as it
/// is asked for. A body over the cap is not sent: its rows are halved and each half encoded again,
/// so the pieces come out in row order. A single row over the cap is sent as it is, for the route
/// to refuse.
fn halved<F>(count: usize, cap: usize, encode: F) -> Halved<F>
where
    F: FnMut(usize, usize) -> Result<Vec<u8>, String>,
{
    let pending = if count > 0 {
        vec![(0, count)]
    } else {
        Vec::new()
    };
    Halved {
        pending,
        cap,
        encode,
    }
}

struct Halved<F> {
    /// The runs still to encode, the next on top.
    pending: Vec<(usize, usize)>,
    cap: usize,
    encode: F,
}

impl<F> Iterator for Halved<F>
where
    F: FnMut(usize, usize) -> Result<Vec<u8>, String>,
{
    type Item = Result<(usize, usize, Vec<u8>), String>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((first, many)) = self.pending.pop() {
            let body = match (self.encode)(first, many) {
                Ok(body) => body,
                Err(e) => {
                    self.pending.clear();
                    return Some(Err(e));
                }
            };
            if body.len() > self.cap && many > 1 {
                let half = many / 2;
                self.pending.push((first + half, many - half));
                self.pending.push((first, half));
                continue;
            }
            return Some(Ok((first, many, body)));
        }
        None
    }
}

// ---------------------------------------------------------------------------------------------
// Reading rows
// ---------------------------------------------------------------------------------------------

/// A file's rows, as Arrow batches or as NDJSON lines.
enum Rows {
    Batches(Box<dyn Iterator<Item = Result<RecordBatch, String>>>),
    /// The lines, and the columns to keep of each.
    Lines(Box<dyn BufRead>, Option<Vec<String>>),
}

/// Up to one page's rows.
enum Page {
    Batch(RecordBatch),
    Lines(Vec<String>),
}

impl Page {
    fn len(&self) -> usize {
        match self {
            Page::Batch(batch) => batch.num_rows(),
            Page::Lines(lines) => lines.len(),
        }
    }

    fn content_type(&self) -> &'static str {
        match self {
            Page::Batch(_) => ARROW_STREAM,
            Page::Lines(_) => NDJSON,
        }
    }

    /// `len` rows from `offset`, as the ingest route takes them.
    fn encode(&self, offset: usize, len: usize) -> Result<Vec<u8>, String> {
        match self {
            Page::Batch(batch) => {
                let mut out = Vec::new();
                {
                    let mut writer = StreamWriter::try_new(&mut out, &batch.schema())
                        .map_err(|e| format!("encoding a page: {e}"))?;
                    writer
                        .write(&batch.slice(offset, len))
                        .and_then(|()| writer.finish())
                        .map_err(|e| format!("encoding a page: {e}"))?;
                }
                Ok(out)
            }
            Page::Lines(lines) => {
                let mut out = Vec::new();
                for line in &lines[offset..offset + len] {
                    out.extend_from_slice(line.as_bytes());
                    out.push(b'\n');
                }
                Ok(out)
            }
        }
    }

    /// The rows as JSON objects, and the columns left out of them because JSON cannot carry
    /// them here. `first` is the first row's position in the input, for a refusal to name.
    fn objects(&self, first: usize) -> Result<(Vec<Value>, Vec<String>), String> {
        match self {
            Page::Batch(batch) => Ok(row_objects(batch)),
            Page::Lines(lines) => lines
                .iter()
                .enumerate()
                .map(|(at, line)| match serde_json::from_str(line) {
                    Ok(Value::Object(row)) => Ok(Value::Object(row)),
                    _ => Err(format!("row {} is not a JSON object", first + at)),
                })
                .collect::<Result<_, _>>()
                .map(|rows| (rows, Vec::new())),
        }
    }
}

fn input_format(path: &Path, flag: Option<InputFormat>) -> Result<InputFormat, String> {
    if let Some(flag) = flag {
        return Ok(flag);
    }
    if path == Path::new("-") {
        return Err("reading stdin needs --format ipc or --format ndjson".into());
    }
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("parquet") => Ok(InputFormat::Parquet),
        Some("arrows" | "arrow") => Ok(InputFormat::Ipc),
        Some("ndjson" | "jsonl") => Ok(InputFormat::Ndjson),
        _ => Err(format!(
            "{} does not end in .parquet, .arrows, .arrow, .ndjson or .jsonl; name one of those, \
             or pass --format",
            path.display()
        )),
    }
}

/// The rows of `path`, or of stdin for `-`, read `batch_rows` at a time where the format allows.
fn open_rows(path: &Path, input: &Input, batch_rows: usize) -> Result<Rows, String> {
    let format = input_format(path, input.format)?;
    let stdin = path == Path::new("-");
    let name = if stdin {
        "stdin".to_string()
    } else {
        path.display().to_string()
    };
    let open = || File::open(path).map_err(|e| format!("opening {name}: {e}"));
    let unreadable = {
        let name = name.clone();
        move |e: &dyn std::fmt::Display| format!("reading {name}: {e}")
    };
    match format {
        InputFormat::Ndjson => {
            let reader: Box<dyn BufRead> = if stdin {
                Box::new(BufReader::new(std::io::stdin()))
            } else {
                Box::new(BufReader::new(open()?))
            };
            Ok(Rows::Lines(reader, input.columns.clone()))
        }
        InputFormat::Parquet => {
            if stdin {
                return Err(
                    "Parquet cannot be read from stdin, since it is read from its end; \
                            pass the file, or send an Arrow IPC stream with --format ipc"
                        .into(),
                );
            }
            let builder =
                ParquetRecordBatchReaderBuilder::try_new(open()?).map_err(|e| unreadable(&e))?;
            let builder = match &input.columns {
                Some(names) => {
                    let mask = ProjectionMask::roots(
                        builder.parquet_schema(),
                        indices(builder.schema(), names, &name)?,
                    );
                    builder.with_projection(mask)
                }
                None => builder,
            };
            let reader = builder
                .with_batch_size(batch_rows)
                .build()
                .map_err(|e| unreadable(&e))?;
            Ok(Rows::Batches(Box::new(
                reader.map(move |batch| batch.map_err(|e| unreadable(&e))),
            )))
        }
        InputFormat::Ipc => {
            let (schema, batches): (_, Box<dyn Iterator<Item = Result<RecordBatch, ArrowError>>>) =
                if stdin {
                    let reader = StreamReader::try_new(BufReader::new(std::io::stdin()), None)
                        .map_err(|e| unreadable(&e))?;
                    (reader.schema(), Box::new(reader))
                } else {
                    let mut file = open()?;
                    let mut magic = [0u8; 6];
                    let is_file = file.read_exact(&mut magic).is_ok() && &magic == b"ARROW1";
                    file.rewind().map_err(|e| unreadable(&e))?;
                    if is_file {
                        let reader = FileReader::try_new(file, None).map_err(|e| unreadable(&e))?;
                        (reader.schema(), Box::new(reader))
                    } else {
                        let reader = StreamReader::try_new(BufReader::new(file), None)
                            .map_err(|e| unreadable(&e))?;
                        (reader.schema(), Box::new(reader))
                    }
                };
            let projection = match &input.columns {
                Some(names) => Some(indices(&schema, names, &name)?),
                None => None,
            };
            Ok(Rows::Batches(Box::new(batches.map(move |batch| {
                let batch = batch.map_err(|e| unreadable(&e))?;
                match &projection {
                    Some(indices) => batch.project(indices).map_err(|e| unreadable(&e)),
                    None => Ok(batch),
                }
            }))))
        }
    }
}

/// Each name's column in `schema`, or a refusal listing the columns there are.
fn indices(schema: &Schema, names: &[String], file: &str) -> Result<Vec<usize>, String> {
    names
        .iter()
        .map(|name| {
            schema.index_of(name).map_err(|_| {
                let held: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
                format!(
                    "{file} has no column `{name}`; name some of {}",
                    held.join(", ")
                )
            })
        })
        .collect()
}

/// Hand `each` the rows a page of `page_rows` at a time, with the first row's position, until
/// the rows end or `each` says to stop.
fn paged(
    rows: Rows,
    page_rows: usize,
    mut each: impl FnMut(usize, Page) -> Result<bool, String>,
) -> Result<(), String> {
    let mut first = 0;
    match rows {
        Rows::Batches(batches) => {
            let mut held: VecDeque<RecordBatch> = VecDeque::new();
            let mut count = 0;
            for batch in batches {
                let batch = batch?;
                count += batch.num_rows();
                held.push_back(batch);
                while count >= page_rows {
                    let page = take(&mut held, page_rows)?;
                    count -= page_rows;
                    if !each(first, Page::Batch(page))? {
                        return Ok(());
                    }
                    first += page_rows;
                }
            }
            if count > 0 {
                each(first, Page::Batch(take(&mut held, count)?))?;
            }
        }
        Rows::Lines(reader, columns) => {
            let mut lines = Vec::with_capacity(page_rows.min(1 << 16));
            for (number, line) in reader.lines().enumerate() {
                let line = line.map_err(|e| format!("reading line {}: {e}", number + 1))?;
                if line.trim().is_empty() {
                    continue;
                }
                lines.push(match &columns {
                    Some(columns) => kept(&line, columns)
                        .ok_or_else(|| format!("line {} is not a JSON object", number + 1))?,
                    None => line,
                });
                if lines.len() == page_rows {
                    let page = std::mem::take(&mut lines);
                    if !each(first, Page::Lines(page))? {
                        return Ok(());
                    }
                    first += page_rows;
                }
            }
            if !lines.is_empty() {
                each(first, Page::Lines(lines))?;
            }
        }
    }
    Ok(())
}

/// The first `n` rows held, as one batch of their own.
fn take(held: &mut VecDeque<RecordBatch>, n: usize) -> Result<RecordBatch, String> {
    let mut parts = Vec::new();
    let mut need = n;
    while need > 0 {
        let Some(front) = held.pop_front() else {
            break;
        };
        if front.num_rows() <= need {
            need -= front.num_rows();
            parts.push(front);
        } else {
            parts.push(front.slice(0, need));
            held.push_front(front.slice(need, front.num_rows() - need));
            need = 0;
        }
    }
    let schema = parts
        .first()
        .map(RecordBatch::schema)
        .ok_or("no rows were held")?;
    arrow::compute::concat_batches(&schema, &parts)
        .map_err(|e| format!("the input's batches do not share one schema: {e}"))
}

/// An NDJSON line with only `columns` kept, or `None` where it is not a JSON object.
fn kept(line: &str, columns: &[String]) -> Option<String> {
    let Ok(Value::Object(mut row)) = serde_json::from_str(line) else {
        return None;
    };
    row.retain(|key, _| columns.contains(key));
    Some(Value::Object(row).to_string())
}

/// A batch's rows as JSON objects, as a change or a vocabulary value names them: an integer or a
/// timestamp as its decimal digits, a string as itself. A column JSON cannot carry here, such as a
/// list, is left out, and named in the second list.
fn row_objects(batch: &RecordBatch) -> (Vec<Value>, Vec<String>) {
    let schema = batch.schema();
    let mut columns = Vec::new();
    let mut skipped = Vec::new();
    for (field, column) in schema.fields().iter().zip(batch.columns()) {
        match cells(column) {
            Some(cells) => columns.push((field.name(), cells)),
            None => skipped.push(field.name().clone()),
        }
    }
    let rows = (0..batch.num_rows())
        .map(|row| {
            Value::Object(
                columns
                    .iter()
                    .map(|(name, cells)| ((*name).clone(), cells[row].clone()))
                    .collect(),
            )
        })
        .collect();
    (rows, skipped)
}

/// A column's cells as JSON, or `None` where JSON cannot carry them here.
fn cells(column: &ArrayRef) -> Option<Vec<Value>> {
    let cast = |array: &ArrayRef, to: &DataType| arrow::compute::cast(array, to).ok();
    let text = |array: &ArrayRef| -> Option<Vec<Value>> {
        Some(
            cast(array, &DataType::Utf8)?
                .as_string::<i32>()
                .iter()
                .map(|cell| cell.map_or(Value::Null, |text| Value::String(text.to_owned())))
                .collect(),
        )
    };
    match column.data_type() {
        DataType::Null => Some(vec![Value::Null; column.len()]),
        DataType::Boolean => Some(
            column
                .as_boolean()
                .iter()
                .map(|cell| cell.map_or(Value::Null, Value::Bool))
                .collect(),
        ),
        DataType::Float32 | DataType::Float64 => Some(
            cast(column, &DataType::Float64)?
                .as_primitive::<Float64Type>()
                .iter()
                .map(|cell| {
                    cell.and_then(serde_json::Number::from_f64)
                        .map_or(Value::Null, Value::Number)
                })
                .collect(),
        ),
        DataType::Timestamp(TimeUnit::Microsecond, _) => text(&cast(column, &DataType::Int64)?),
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => text(column),
        DataType::Dictionary(_, values)
            if matches!(**values, DataType::Utf8 | DataType::LargeUtf8) =>
        {
            text(column)
        }
        integer if integer.is_integer() => text(column),
        _ => None,
    }
}

/// The `--ids-out` file: each row's position and the `mosaica_id` its answer gave it.
struct Ids {
    output: Output,
    schema: Arc<Schema>,
    written: bool,
}

impl Ids {
    fn new(path: PathBuf) -> Result<Ids, String> {
        // `Output::file` refuses only a path whose extension names no format.
        let output = Output::file(path.clone()).map_err(|_| {
            format!(
                "--ids-out {} names no file ending in .arrows or .parquet; name one, since the \
                 extension says which to write and stdout carries the report",
                path.display()
            )
        })?;
        Ok(Ids {
            output,
            schema: Arc::new(Schema::new(vec![
                Field::new("row", DataType::UInt64, false),
                Field::new("mosaica_id", DataType::UInt64, true),
            ])),
            written: false,
        })
    }

    fn write(&mut self, first: usize, count: usize, answer: Option<&Value>) -> Result<(), String> {
        let rows = UInt64Array::from_iter_values((first..first + count).map(|row| row as u64));
        let ids: UInt64Array = (0..count)
            .map(|at| {
                answer
                    .and_then(|answer| answer["mosaica_ids"][at].as_str())
                    .and_then(|id| id.parse::<u64>().ok())
            })
            .collect();
        let batch = RecordBatch::try_new(
            Arc::clone(&self.schema),
            vec![Arc::new(rows) as ArrayRef, Arc::new(ids)],
        )
        .map_err(|e| e.to_string())?;
        self.written = true;
        self.output.write(&batch)
    }

    fn finish(mut self) -> Result<(), String> {
        if !self.written {
            self.write(0, 0, None)?;
        }
        self.output.finish()
    }
}
