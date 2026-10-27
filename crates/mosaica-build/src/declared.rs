//! What `mosaica build` builds from a parsed declaration: the build's arguments, and the frames it
//! fits once the identity pass has run.
//!
//! A declaration that names no file is a build of no rows: every view, group, attribute and layer
//! it declares, empty, ready to take its rows at a running service.

use std::collections::BTreeMap;
use std::path::PathBuf;

use mosaica_types::IdentityKey;

use crate::config::{ArtifactSource, Config, FrameSource};
use crate::disclosure::Disclosure;
use crate::error::Result;
use crate::{BuildArgs, Framing, ScopedColumnFamily, ScopedLayer, ViewArgs};

/// A declaration's build, before the invocation's own settings.
pub struct DeclaredBuild {
    /// Every argument the declaration decides. `strict`, `emit_oracle_pairs`, `batch_items`,
    /// `memory_budget` and `band_rows` are their defaults, for the invocation to set.
    pub args: BuildArgs,
    /// Under `limit`, the frames fitted over the rows the identity pass keeps. Empty otherwise,
    /// every frame being resolved here and printed.
    pub framings: Vec<Framing>,
    /// What the declaration discloses, read before it was broken up into arguments.
    pub disclosure: Disclosure,
}

/// The build `config` declares, into `out`, keeping under `limit` the items whose value of the
/// declaration's one unique integer attribute is below it, its `mosaica_id`s drawn under
/// `identity_key`.
///
/// Every frame not fitted after the identity pass is resolved here, before any work, and its
/// report printed: `auto` fits the box, and every other spelling is surveyed against the rows it
/// will place.
pub fn declared_build(
    config: Config,
    out: PathBuf,
    limit: Option<u64>,
    identity_key: IdentityKey,
) -> Result<DeclaredBuild> {
    let crate::config::Registry {
        views: registry,
        anchor,
    } = config.registry()?;
    let limited = crate::ids::Limit::of(&config.schema, limit)?.is_some();
    let acquired = config.acquire();
    let acquired_views: Vec<_> = registry.iter().map(crate::config::acquire_view).collect();

    // One frame per view, except on a group, where one frame covers every view of it: `auto` is
    // fitted over the union of their sources and a stated extent is surveyed against every one of
    // them. The views of a group are contiguous in the registry. Under `limit` each frame is fitted
    // once the identity pass has decided which rows it keeps, and each view holds a placeholder
    // until then.
    let mut framings: Vec<Framing> = Vec::new();
    let mut extents: Vec<mosaica_spatial::Bounds> = Vec::with_capacity(registry.len());
    let mut frame_of: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, view) in registry.iter().enumerate() {
        let owner = match &view.group {
            Some(membership) => membership.group.as_str(),
            None => view.id.as_str(),
        };
        match frame_of.get(owner) {
            Some(&first) => {
                extents.push(extents[first]);
                continue;
            }
            None => frame_of.insert(owner, index),
        };
        let members: Vec<usize> = registry
            .iter()
            .enumerate()
            .filter(|(_, v)| match (&v.group, &view.group) {
                (Some(a), Some(b)) => a.group == b.group,
                _ => v.id == view.id,
            })
            .map(|(i, _)| i)
            .collect();
        let subject = match &view.group {
            Some(membership) => format!("view group '{}'", membership.group),
            None => format!("view '{}'", view.id),
        };
        if limited {
            framings.push(Framing {
                subject,
                projection: view.projection,
                extent: view.extent,
                views: members,
            });
            extents.push(mosaica_spatial::Bounds {
                x_min: 0.0,
                x_max: 1.0,
                y_min: 0.0,
                y_max: 1.0,
            });
            continue;
        }
        // A view that names no file gives the frame no rows.
        let sources: Vec<FrameSource> = members
            .iter()
            .filter_map(|&i| {
                let acquired = &acquired_views[i];
                Some(FrameSource {
                    points: acquired.points.as_deref()?,
                    fields: &acquired.point_fields,
                    select: acquired.select.as_ref(),
                    kept: None,
                })
            })
            .collect();
        let frame =
            crate::config::frame_of(&subject, view.projection, &view.extent, &sources)?;
        eprintln!("{}", frame.report());
        extents.push(frame.extent);
    }

    let views: Vec<ViewArgs> = registry
        .iter()
        .zip(acquired_views)
        .zip(&extents)
        .map(|((view, acquired), extent)| ViewArgs {
            // This view's own gate: the plain view's `visibility`, or for a view of a group its
            // roster record's own. The group's half travels on the group descriptor beside it.
            visibility: view.visibility.clone(),
            view_id: view.id.clone(),
            title: view.title.clone(),
            projection: view.projection,
            extent: *extent,
            points: acquired.points,
            point_fields: acquired.point_fields,
            select: acquired.select,
            access: acquired.access,
        })
        .collect();
    // One column per view of the group: the views a family covers are the ones its group owns,
    // and each of them already says where its rows are.
    let scoped_attributes: Vec<ScopedColumnFamily> = config
        .scoped_attributes
        .iter()
        .map(|scoped| ScopedColumnFamily {
            attribute: scoped.attribute.clone(),
            group: scoped.group.clone(),
            views: registry
                .iter()
                .enumerate()
                .filter(|(_, view)| {
                    view.group
                        .as_ref()
                        .is_some_and(|group| group.group == scoped.group)
                })
                .map(|(index, _)| index)
                .collect(),
            source: scoped.source.clone(),
        })
        .collect();
    // A scoped layer is a different artifact set per view of one group: its rows say which view
    // each artifact belongs to, under the layer's own `fields.view`, and the group's keys are what
    // a stray value is refused against.
    let scoped_layers: BTreeMap<String, ScopedLayer> = config
        .scopes
        .layers
        .iter()
        .map(|(layer, group)| {
            let mut keys: Vec<String> = registry
                .iter()
                .filter_map(|view| view.group.as_ref())
                .filter(|membership| {
                    &membership.group == group || membership.members_of.as_ref() == Some(group)
                })
                .map(|membership| membership.key.clone())
                .collect();
            keys.sort();
            keys.dedup();
            let column = config
                .layer_sources
                .iter()
                .find(|source| &source.name == layer)
                .and_then(|source| match &source.artifacts {
                    Some(ArtifactSource::File { fields, .. }) => Some(fields.of("view").to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| "view".to_string());
            (
                layer.clone(),
                ScopedLayer {
                    group: group.clone(),
                    column,
                    keys,
                },
            )
        })
        .collect();
    let disclosure = Disclosure::of(&config);
    let groups = config.group_registry(&registry, &views)?;
    let args = BuildArgs {
        views,
        anchor,
        groups,
        scoped_attributes,
        attribute_sources: acquired.attribute_sources,
        out,
        limit,
        strict: false,
        identity_key,
        emit_oracle_pairs: true,
        batch_items: None,
        memory_budget: None,
        band_rows: None,
        schema: config.schema,
        layers: config.layers,
        layer_inputs: acquired.layers,
        scoped_layers,
        shard_size: config.shard_size,
    };
    Ok(DeclaredBuild {
        args,
        framings,
        disclosure,
    })
}
