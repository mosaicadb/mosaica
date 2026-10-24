//! `POST /v1/aggregate`: how the items a viewer may see in one view are distributed, as one table
//! per grouping of the figures it names, framed as `/v1/items` frames its pages with a table head
//! before each table's first page in a response.
//!
//! It is sent beside viewport requests, so it runs under the viewport's admission. Its response is
//! held to its own byte budget, `serve.aggregate_response_bytes`, in pages of
//! `serve.aggregate_page_bytes`, and to the bulk reads' time budget, and ends with a cursor where
//! they cut it; a cell level lists at most `selection.max_aggregate_cells` cells, and a histogram
//! at most `selection.max_aggregate_bins` bins.

use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use serde::Deserialize;
use serde_json::Value;

use mosaica_engine::{
    AggregateCaps, AggregateHead, AggregateRefused, AggregateRequest, AggregateSink, By,
    CancelToken, Cut, EngineError, Figure, FigureField, FigureList, Grouping, PageEnd, Pick,
    RecordsLimits, RecordsSink, RecordsTrailer, Reference, SinkResult, TableHead,
};
use mosaica_engine::filter::Family;
use mosaica_wire::table_head_frame;

use crate::control::present;
use crate::error::ApiError;
use crate::records::{bulk_read, limits, view_and_filter, CompressionReq, FrameSink, Lane, Opening, Read};
use crate::state::{ApiJson, AppState, ViewerSession};
use crate::viewer::{check_bbox, field_column, FieldColumn, FilterParser};

/// The request body. Every field but `view` and `groupings` may be left out; an unknown one is a
/// `422`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AggregateReq {
    view: String,
    #[serde(default)]
    filters: Option<Value>,
    /// `{}` is the whole visible set.
    #[serde(default)]
    reference: Option<Value>,
    groupings: Vec<GroupingReq>,
    #[serde(default)]
    page_rows: Option<u32>,
    #[serde(default)]
    pages: Option<u32>,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    compression: Option<CompressionReq>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupingReq {
    #[serde(default)]
    by: Option<ByReq>,
    #[serde(default)]
    cells: Option<CellsReq>,
    /// Only `true`.
    #[serde(default, deserialize_with = "present")]
    count: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    values: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present")]
    missing: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present")]
    min: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present")]
    max: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present")]
    mean: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present")]
    summary: Option<Vec<String>>,
}

impl GroupingReq {
    /// The field figures' lists in the order the engine orders their columns by.
    fn figure_lists(&self) -> [(Figure, &Option<Vec<String>>); 6] {
        [
            (Figure::Values, &self.values),
            (Figure::Missing, &self.missing),
            (Figure::Min, &self.min),
            (Figure::Max, &self.max),
            (Figure::Mean, &self.mean),
            (Figure::Summary, &self.summary),
        ]
    }
}

/// A grouping's outer level: `field` with `top` or `values`, or with `bins` and optionally
/// `range`; or `layer` with `top` or `artifacts` and, on a levelled layer, `level`, or
/// on a treed layer, `cut`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ByReq {
    #[serde(default)]
    field: Option<String>,
    #[serde(default)]
    layer: Option<String>,
    #[serde(default)]
    level: Option<u32>,
    #[serde(default)]
    top: Option<u32>,
    #[serde(default, deserialize_with = "present")]
    values: Option<Vec<String>>,
    #[serde(default)]
    bins: Option<u32>,
    /// `[lower, upper]`, each a number, or on an integer or timestamp field its decimal string,
    /// and on a timestamp field an RFC 3339 date or date-time with an offset.
    #[serde(default)]
    range: Option<[Value; 2]>,
    /// With `bins`: count about this many of the set's items and scale the counts to the set.
    #[serde(default)]
    sample: Option<u64>,
    /// `mosaica_id`s, each a number or its decimal string.
    #[serde(default)]
    artifacts: Option<Vec<Value>>,
    /// On a treed layer: list the artifacts the map draws at this zoom, bbox and budget.
    #[serde(default)]
    cut: Option<CutReq>,
    /// With `layer`: each listed artifact carries this viewer's `slot` in a palette this size.
    #[serde(default)]
    palette_size: Option<u32>,
}

/// As `POST /v1/artifacts/viewport` names them.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CutReq {
    zoom: u8,
    bbox: [f64; 4],
    #[serde(default)]
    budget: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CellsReq {
    depth: u64,
    /// As the viewport's `bbox`; absent is the view's whole extent.
    #[serde(default)]
    area: Option<[f64; 4]>,
}

/// The table heads and pages of an aggregate response. It has no head frame of its own: the
/// engine's head carries only what the response headers need.
impl AggregateSink for FrameSink {
    fn head(&mut self, head: &AggregateHead) -> SinkResult {
        self.producer.open(Opening {
            identity_key: head.identity_key,
            region: head.region,
            head: Vec::new(),
            server_us: self.start.elapsed().as_micros() as u64,
        })
    }

    fn table(&mut self, head: &TableHead) -> SinkResult {
        let mut json = serde_json::json!({
            "grouping": head.grouping,
            "total": head.total,
            "resumed": head.resumed,
        });
        if let Some(total) = head.reference_total {
            json["reference_total"] = total.into();
        }
        if let Some(groups) = head.groups {
            json["groups"] = groups.into();
        }
        if let Some(sample) = head.sample {
            json["sample"] = serde_json::json!({
                "sampled": sample.sampled,
                "items": sample.items,
            });
            if let Some(items) = sample.reference_items {
                json["sample"]["reference_items"] = items.into();
            }
        }
        self.producer
            .send(table_head_frame(json.to_string().as_bytes()))
    }

    fn page(
        &mut self,
        _grouping: u32,
        batch: &arrow::record_batch::RecordBatch,
        end: &PageEnd,
    ) -> SinkResult {
        RecordsSink::page(self, batch, end)
    }
}

/// `POST /v1/aggregate`.
pub(crate) async fn aggregate(
    State(state): State<Arc<AppState>>,
    ViewerSession(session): ViewerSession,
    ApiJson(req): ApiJson<AggregateReq>,
) -> Result<Response, ApiError> {
    let compression = req.compression;
    let read = move |state: &AppState, session: &mosaica_engine::Session, cancel, sink: &mut _| {
        run_aggregate(state, session, req, cancel, sink)
    };
    bulk_read(state, session, Lane::Compute, "aggregate", "", compression, read).await
}

fn run_aggregate(
    state: &AppState,
    session: &mosaica_engine::Session,
    req: AggregateReq,
    cancel: CancelToken,
    sink: &mut FrameSink,
) -> Read {
    let meta = state.engine.meta();
    let resolved = view_and_filter(state, &meta, session, &req.view, req.filters.as_ref())
        .and_then(|(view, filter)| {
            let reference = match &req.reference {
                None => None,
                Some(Value::Object(empty)) if empty.is_empty() => Some(Reference::Visible),
                Some(value) => Some(Reference::Filter(
                    FilterParser::new(
                        &meta,
                        view,
                        session.visible_views(),
                        state.limits.max_region_vertices,
                    )
                    .parse(value)?,
                )),
            };
            let groupings = req
                .groupings
                .iter()
                .map(|grouping| grouping_of(&meta, &view.id, session, grouping))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((view, filter, reference, groupings))
        });
    let (view, filter, reference, groupings) = match resolved {
        Ok(resolved) => resolved,
        Err(e) => return Read::Refused(e),
    };
    let request = AggregateRequest {
        view: &view.id,
        filter,
        reference,
        groupings: &groupings,
        page_rows: req.page_rows,
        pages: req.pages,
        cursor: req.cursor.as_deref(),
        limits: RecordsLimits {
            max_page_bytes: state.limits.aggregate_page_bytes,
            response_bytes: state.limits.aggregate_response_bytes,
            ..limits(state)
        },
        caps: AggregateCaps {
            groupings: state.limits.max_aggregate_groupings,
            top: state.limits.max_aggregate_top,
            named: state.limits.max_aggregate_named,
            bins: state.limits.max_aggregate_bins,
            cells: state.limits.max_aggregate_cells,
        },
        cancel: Some(cancel),
    };
    let mut recomposed = false;
    let outcome = state
        .engine
        .aggregate_stream(session, request, sink)
        .map(|trailer| {
            recomposed = trailer.recomposed;
            RecordsTrailer {
                pages: trailer.pages,
                rows: trailer.rows,
                next: trailer.next,
                ended_by: trailer.ended_by,
            }
        })
        .map_err(|e| in_callers_words(e, &req.groupings, &groupings));
    Read::Ran {
        view: view.id.clone(),
        outcome,
        recomposed,
    }
}

/// One grouping as the engine takes it, with its field resolved under `view`.
fn grouping_of(
    meta: &mosaica_engine::EngineMeta,
    view: &str,
    session: &mosaica_engine::Session,
    grouping: &GroupingReq,
) -> Result<Grouping, ApiError> {
    let by = match &grouping.by {
        None => None,
        Some(by) => Some(by_of(meta, view, session, by)?),
    };
    let area = match &grouping.cells {
        Some(CellsReq {
            area: Some(area), ..
        }) => {
            check_bbox("area", area)?;
            Some(*area)
        }
        _ => None,
    };
    let cells = match &grouping.cells {
        None => None,
        Some(CellsReq { depth, .. }) => match u8::try_from(*depth) {
            Ok(cells) if cells <= 32 => Some(cells),
            _ => {
                return Err(ApiError::Contract(format!(
                    "cells depth {depth} is past the stored resolution; ask for a depth from 0 \
                     to 32"
                )))
            }
        },
    };
    let count = match grouping.count {
        None => false,
        Some(true) => true,
        Some(false) => {
            return Err(ApiError::Contract(
                "`count` is only ever `true`; leave it out for a table without counts"
                    .to_string(),
            ))
        }
    };
    let mut figures = Vec::new();
    for (figure, fields) in grouping.figure_lists() {
        let Some(fields) = fields else { continue };
        let fields = fields
            .iter()
            .map(|field| figure_field(meta, view, session, field))
            .collect::<Result<_, _>>()?;
        figures.push(FigureList { figure, fields });
    }
    Ok(Grouping {
        by,
        count,
        figures,
        cells,
        area,
    })
}

/// A field a figure is named over, resolved under `view`. Whether it has figures is the engine's
/// to decide.
fn figure_field(
    meta: &mosaica_engine::EngineMeta,
    view: &str,
    session: &mosaica_engine::Session,
    field: &str,
) -> Result<FigureField, ApiError> {
    let column = resolved_column(meta, view, session, field)?.0;
    Ok(FigureField {
        column,
        name: field.to_string(),
    })
}

/// `field`'s column under `view`, its family and whether it is an integer.
fn resolved_column(
    meta: &mosaica_engine::EngineMeta,
    view: &str,
    session: &mosaica_engine::Session,
    field: &str,
) -> Result<(String, Family, Option<mosaica_types::written::WrittenAs>), ApiError> {
    match field_column(meta, field, view, session.visible_views())? {
        FieldColumn::Resolved {
            column,
            family,
            integer,
        } => Ok((column, family, integer)),
        FieldColumn::Unknown => Err(ApiError::Contract(format!(
            "field '{field}' is unknown; name a field /v1/meta publishes"
        ))),
        FieldColumn::Unpinned { group } => Err(ApiError::Contract(format!(
            "field '{field}' is scoped to view group '{group}' and view '{view}' is not one of \
             its views; pin the view it means as '{field}@<key>'"
        ))),
    }
}

fn by_of(
    meta: &mosaica_engine::EngineMeta,
    view: &str,
    session: &mosaica_engine::Session,
    by: &ByReq,
) -> Result<By, ApiError> {
    let bad = |detail: &str| Err(ApiError::Contract(detail.to_string()));
    match (&by.field, &by.layer) {
        (Some(field), None) => {
            if by.artifacts.is_some() || by.level.is_some() || by.cut.is_some() {
                return bad(
                    "`artifacts`, `level` and `cut` go with `layer`; a field takes `top`, \
                     `values` or `bins`",
                );
            }
            if by.range.is_some() && by.bins.is_none() {
                return bad("`range` goes with `bins`; send `bins` beside it, or leave it out");
            }
            if by.sample.is_some() && by.bins.is_none() {
                return bad("`sample` goes with `bins`; send `bins` beside it, or leave it out");
            }
            let pick = match (by.bins, by.top, &by.values) {
                (Some(_), None, None) => None,
                (Some(_), _, _) => {
                    return bad("`by` carries `bins` beside `top` or `values`; send one")
                }
                (None, Some(top), None) => Some(Pick::Top(top)),
                (None, None, Some(values)) => Some(Pick::Named(values.clone())),
                (None, Some(_), Some(_)) => {
                    return bad("`by` carries both `top` and `values`; send one")
                }
                (None, None, None) => {
                    return bad(
                        "`by` on a field carries none of `top`, `values` and `bins`; send one",
                    )
                }
            };
            let (column, family, integer) = resolved_column(meta, view, session, field)?;
            match (pick, by.bins, family) {
                (Some(pick), _, Family::Category) => Ok(By::Field { column, pick }),
                (Some(_), _, _) => Err(ApiError::Contract(format!(
                    "field '{field}' is not a category; name a category field"
                ))),
                (None, Some(bins), Family::Numeric) => {
                    let range = match &by.range {
                        None => None,
                        Some([lower, upper]) => Some((
                            crate::filter_dto::numeric_value(field, lower, integer)?,
                            crate::filter_dto::numeric_value(field, upper, integer)?,
                        )),
                    };
                    Ok(By::Bins {
                        column,
                        bins,
                        range,
                        sample: by.sample,
                    })
                }
                (None, _, Family::Category) => Err(ApiError::Contract(format!(
                    "field '{field}' is a category; bins go with a number or timestamp field, \
                     and a category takes top or values"
                ))),
                (None, _, _) => Err(ApiError::Contract(format!(
                    "field '{field}' is not a number or timestamp; name a number or timestamp \
                     field for bins"
                ))),
            }
        }
        (None, Some(layer)) => {
            if by.values.is_some()
                || by.bins.is_some()
                || by.range.is_some()
                || by.sample.is_some()
            {
                return bad(
                    "`values`, `bins`, `range` and `sample` go with `field`; a layer takes `top` \
                     or `artifacts`",
                );
            }
            let palette = by
                .palette_size
                .map(mosaica_engine::check_palette_size)
                .transpose()
                .map_err(crate::error::map_engine_error)?;
            let pick = match (by.top, &by.artifacts) {
                (Some(top), None) => Pick::Top(top),
                (None, Some(ids)) => Pick::Named(
                    ids.iter()
                        .map(|id| crate::filter_dto::mosaica_id(Some(id), "artifacts"))
                        .collect::<Result<_, _>>()?,
                ),
                (Some(_), Some(_)) => {
                    return bad("`by` carries both `top` and `artifacts`; send one")
                }
                (None, None) => return bad("`by` carries neither `top` nor `artifacts`; send one"),
            };
            let cut = match &by.cut {
                None => None,
                Some(_) if by.top.is_none() => {
                    return bad("`cut` goes with `top`; send `top` beside it, or leave it out")
                }
                Some(cut) => {
                    check_bbox("cut.bbox", &cut.bbox)?;
                    Some(Cut {
                        zoom: cut.zoom,
                        bbox: cut.bbox,
                        budget: cut.budget,
                    })
                }
            };
            Ok(By::Layer {
                layer: layer.clone(),
                level: by.level,
                pick,
                cut,
                palette,
            })
        }
        (Some(_), Some(_)) => bad("`by` names both `field` and `layer`; name one"),
        (None, None) => bad("`by` names neither `field` nor `layer`; name one"),
    }
}

/// A refusal naming a field the engine was given resolved, named as the caller spelled it.
fn in_callers_words(e: EngineError, asked: &[GroupingReq], sent: &[Grouping]) -> EngineError {
    let (column, rename): (&String, fn(String) -> AggregateRefused) = match &e {
        EngineError::AggregateRefused(AggregateRefused::NotCountable(column)) => {
            (column, AggregateRefused::NotCountable)
        }
        EngineError::AggregateRefused(AggregateRefused::NotBinnable(column)) => {
            (column, AggregateRefused::NotBinnable)
        }
        EngineError::AggregateRefused(AggregateRefused::BinsOnBool(column)) => {
            (column, AggregateRefused::BinsOnBool)
        }
        EngineError::AggregateRefused(AggregateRefused::FractionalTime(column)) => {
            (column, AggregateRefused::FractionalTime)
        }
        _ => return e,
    };
    let spelling = asked.iter().zip(sent).find_map(|(asked, sent)| match &sent.by {
        Some(By::Field {
            column: resolved, ..
        })
        | Some(By::Bins {
            column: resolved, ..
        }) if resolved == column =>
        {
            asked.by.as_ref().and_then(|by| by.field.clone())
        }
        _ => None,
    });
    match spelling {
        Some(field) => EngineError::AggregateRefused(rename(field)),
        None => e,
    }
}
