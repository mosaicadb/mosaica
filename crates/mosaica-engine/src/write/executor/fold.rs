use super::*;

/// Memory this process could take without reclaiming anything it needs, in bytes. `None` where
/// unknowable.
///
/// The smaller of `MemAvailable` and a cgroup v2 `memory.max`: either can be the real bound.
pub(super) fn available_memory() -> Option<u64> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    let available = meminfo
        .lines()
        .find(|line| line.starts_with("MemAvailable:"))
        .and_then(|line| line.split_whitespace().nth(1)?.parse::<u64>().ok())
        .map(|kib| kib * 1024)?;

    // "max" (unlimited) parses to `None`, leaving `MemAvailable` as the answer.
    let cgroup = std::fs::read_to_string("/proc/self/cgroup")
        .ok()
        .and_then(|cgroup| {
            let path = cgroup
                .lines()
                .next()?
                .split(':')
                .nth(2)?
                .trim_start_matches('/');
            std::fs::read_to_string(format!("/sys/fs/cgroup/{path}/memory.max")).ok()
        })
        .and_then(|max| max.trim().parse::<u64>().ok());

    Some(cgroup.map_or(available, |limit| limit.min(available)))
}

/// The levels a fold's retirement is about to move, and the set it retires.
///
/// A fold writes its manifest before it retires, since the retirement is not reversible: a
/// manifest that would not commit must leave it undone. [`Self::records`] composes a level's
/// records without the retired artifacts, and [`Self::version_after`] stamps them with the version
/// the level will carry in a shard once the retirement has run. A containment partition and a
/// spatial level's row forms are omitted for a pending level and recompose on first use.
pub(super) struct PendingRetirement {
    levels: Vec<mosaica_lifecycle::membership::MovedLevel>,
    retired: mosaica_lifecycle::membership::Retired,
}

impl PendingRetirement {
    fn moved(&self, layer: &str, level: u32) -> Option<&mosaica_lifecycle::membership::MovedLevel> {
        self.levels
            .iter()
            .find(|moved| moved.layer == layer && moved.level == level)
    }

    /// Whether the retirement moves `layer`'s `level` in the compacted shard.
    fn is_pending(&self, layer: &str, level: u32) -> bool {
        self.moved(layer, level).is_some()
    }

    /// The version `layer`'s `level` will have in `shard`'s slice of `store` once this fold's
    /// retirement has run, or `None` where `store` holds no slice of `shard`.
    pub(super) fn version_after(
        &self,
        store: &ArtifactStore,
        shard: ShardId,
        layer: &str,
        level: u32,
    ) -> Option<u64> {
        let moves = self.moved(layer, level).is_some_and(|moved| {
            moved.moves == mosaica_lifecycle::membership::Moves::Records
                || shard == self.retired.shard
        });
        Some(store.slice(shard)?.level_version(layer, level) + u64::from(moves))
    }

    /// The records version `layer`'s `level` will have once this fold's retirement has run.
    pub(super) fn records_version_after(
        &self,
        store: &ArtifactStore,
        layer: &str,
        level: u32,
    ) -> u64 {
        let moves = self
            .moved(layer, level)
            .is_some_and(|moved| moved.moves == mosaica_lifecycle::membership::Moves::Records);
        store.records_version(layer, level) + u64::from(moves)
    }

    /// The level's records as the retirement will leave them: every artifact but those whose own
    /// entity is retired.
    fn records<'s>(
        &'s self,
        store: &'s ArtifactStore,
        layer: &str,
        level: u32,
    ) -> impl Iterator<Item = (u32, &'s mosaica_lifecycle::membership::ArtifactRecord)> + 's {
        let retired = store
            .space_of(layer)
            .and_then(|space| self.retired.artifacts.get(&space));
        store.level(layer, level).filter(move |(_, record)| {
            !retired.is_some_and(|retired| retired.contains(record.entity.raw() as u32))
        })
    }

    /// Each of [`Self::records`] as `shard`'s slice holds it, leaving out an artifact the slice
    /// holds nothing of or the retirement releases from it.
    fn held<'s>(
        &'s self,
        store: &'s ArtifactStore,
        shard: ShardId,
        layer: &'s str,
        level: u32,
    ) -> impl Iterator<Item = (u32, &'s mosaica_lifecycle::membership::ArtifactRecord)> + 's {
        let slots = store
            .slice(shard)
            .map_or(&[][..], |slice| slice.slots(layer, level));
        let released = store
            .space_of(layer)
            .filter(|_| shard == self.retired.shard)
            .and_then(|space| self.retired.released.get(&space));
        self.records(store, layer, level)
            .filter_map(move |(ordinal, record)| {
                if released.is_some_and(|released| released.contains(record.entity.raw() as u32)) {
                    return None;
                }
                let held = slots.get(ordinal as usize)?.as_deref()?;
                Some((ordinal, held))
            })
    }
}

/// How many ordinals a level's derived structures cover: one past the highest live ordinal,
/// which is the length the reader sizes the level at (`ArtifactRows::build_over`). A hole below
/// it is covered and a hole at the top is not.
fn level_length<'a>(
    records: impl Iterator<Item = (u32, &'a mosaica_lifecycle::membership::ArtifactRecord)>,
) -> u32 {
    records.map(|(ordinal, _)| ordinal + 1).max().unwrap_or(0)
}

/// What a fold hands the manifest commit in place of the held list: the derived files it has just
/// written, and the retirement its levels are stamped for.
pub(super) struct FoldDerived<'a> {
    /// The shard the fold compacts, which holds what it wrote but its held shapes.
    pub(super) shard: ShardId,
    pub(super) written: &'a [mosaica_store::manifest::DerivedExtent],
    /// The derived files of every other shard the fold links into its prefix.
    pub(super) kept: Vec<(ShardId, mosaica_store::manifest::DerivedExtent)>,
    /// Each layer space's record packs, rewritten into the new prefix.
    pub(super) records: &'a std::collections::BTreeMap<
        LayerSpace,
        Vec<mosaica_store::manifest::RecordPackExtent>,
    >,
    /// Each layer space's content extents in the new prefix, without its deleted artifacts' rows.
    pub(super) contents: &'a super::publications::SpaceContents,
    pub(super) pending_retirement: &'a PendingRetirement,
    /// The ids this fold frees, held back from the WAL position beside them.
    pub(super) freed: &'a mosaica_store::manifest::HeldEntities,
}

/// What a fold frees and retires, by the rule [`freeing`] states.
struct Freeing {
    /// The ids the allocator may issue again, each at the tenancy it is issued at.
    ids: mosaica_lifecycle::ByTenancy,
    /// The numbers among them, each one tenancy higher in the index the fold publishes.
    raised: croaring::Bitmap,
    /// The numbers whose last entity the fold removes at the highest tenancy, never issued again.
    retired: croaring::Bitmap,
}

/// What the fold that removes `executed` from `shard` frees.
///
/// An executed entity an edit gave an item is no item's number, and is freed at tenancy 0. Its
/// number, or the entity itself where it is a number, is freed when the fold removes the number's
/// last entity: the number and every entity the edited-items map pairs with it is executed here
/// or already gone, holding no row, buffered or flushed, and no deny record. Until then the number
/// resolves through the map, so its next holder would resolve to an entity of its last.
///
/// Read before the swap, while the map still holds the pairs of the entities this fold removes,
/// and on the executor thread, so no window commits between this read and the swap.
///
/// A restart before the log rotates past this fold has the next fold free its numbers again one
/// tenancy higher, skipping a tenancy no item was given, since the ids are held until that rotation.
fn freeing(
    shard: &Shard,
    executed: &croaring::Bitmap,
) -> Result<Freeing, mosaica_store::StoreError> {
    const CHUNK: usize = 1 << 16;
    let mut left = croaring::Bitmap::new();
    let mut candidates = croaring::Bitmap::new();
    let mut entities: Vec<EntityId> = Vec::with_capacity(CHUNK);
    let mut iter = executed.iter().peekable();
    while iter.peek().is_some() {
        entities.clear();
        entities.extend(
            iter.by_ref()
                .take(CHUNK)
                .map(|entity| EntityId::new(u64::from(entity))),
        );
        let numbers = crate::edited::numbers_of(shard, &entities)?;
        for (entity, number) in entities.iter().zip(numbers) {
            if number != *entity {
                left.add(entity.raw() as u32);
            }
            candidates.add(number.raw() as u32);
        }
    }

    let gone = |entity: EntityId| {
        executed.contains(entity.raw() as u32)
            || (!shard.overlay.touches(entity) && !crate::edited::holds(shard, entity))
    };
    let mut numbers = croaring::Bitmap::new();
    let mut chunk: Vec<u32> = Vec::with_capacity(CHUNK);
    let mut iter = candidates.iter().peekable();
    while iter.peek().is_some() {
        chunk.clear();
        chunk.extend(iter.by_ref().take(CHUNK));
        let entries = crate::edited::entries_of(shard, &chunk)?;
        for (number, entries) in chunk.iter().zip(entries) {
            if gone(EntityId::new(u64::from(*number))) && entries.into_iter().all(gone) {
                numbers.add(*number);
            }
        }
    }

    let mut freed = Freeing {
        ids: mosaica_lifecycle::ByTenancy::new(),
        raised: croaring::Bitmap::new(),
        retired: croaring::Bitmap::new(),
    };
    if !left.is_empty() {
        freed.ids.insert(mosaica_types::Tenancy::ZERO, left);
    }
    for (tenancy, at) in shard.tenancy.split(&numbers) {
        match tenancy.next() {
            Some(next) => {
                freed.raised.or_inplace(&at);
                freed.ids.entry(next).or_default().or_inplace(&at);
            }
            None => freed.retired.or_inplace(&at),
        }
    }
    Ok(freed)
}

/// The tenancy index a fold publishes.
struct NextTenancy {
    /// The live index's files for the bits the fold leaves as they are, which it links.
    shared: Vec<mosaica_store::manifest::TenancyBit>,
    /// The files it wrote for the bits it changes, with their paths, which it digests.
    written: Vec<(mosaica_store::manifest::TenancyBit, PathBuf)>,
}

impl NextTenancy {
    /// Every bit's entry, as the side-manifest names them.
    fn index(&self) -> Vec<mosaica_store::manifest::TenancyBit> {
        let mut index: Vec<mosaica_store::manifest::TenancyBit> = self
            .shared
            .iter()
            .cloned()
            .chain(self.written.iter().map(|(entry, _)| entry.clone()))
            .collect();
        index.sort_by_key(|entry| entry.bit);
        index
    }
}

/// Write the tenancy index a fold publishes into `prefix_dir`: each bit `raised` changes as a new
/// file, fsynced, and each it leaves shared with `live`.
fn write_next_tenancy_index(
    prefix_dir: &Path,
    layout: ShardLayout<'_>,
    live: &[mosaica_store::manifest::TenancyBit],
    raised: &[Option<croaring::Bitmap>; mosaica_store::tenancy::TENANCY_BITS],
) -> Result<NextTenancy, mosaica_store::StoreError> {
    use mosaica_store::tenancy::{file_name, write_bit};
    let dir_rel = layout.tenancy_rel();
    let mut next = NextTenancy {
        shared: live
            .iter()
            .filter(|entry| matches!(raised.get(entry.bit as usize), Some(None)))
            .cloned()
            .collect(),
        written: Vec::new(),
    };
    for (bit, numbers) in raised.iter().enumerate() {
        let Some(numbers) = numbers else { continue };
        let bit = bit as u32;
        if let Some(path) = write_bit(&prefix_dir.join(&dir_rel), bit, numbers)? {
            let path_rel = format!("{dir_rel}/{}", file_name(bit));
            let entry = mosaica_store::manifest::TenancyBit {
                bit,
                path: path_rel,
            };
            next.written.push((entry, path));
        }
    }
    if !next.written.is_empty() {
        mosaica_store::fsync_dir(&prefix_dir.join(&dir_rel))?;
    }
    Ok(next)
}

/// The derived files of the fold's manifest that stand at their levels' versions once its
/// retirement has run ([`DerivedExtent::stands`], with the layers in `enumerated`). The manifest
/// already left out every file that would not stand at the versions [`PendingRetirement`]
/// predicted, so a file dropped here is a retirement that moved a level other than predicted: a
/// fault, and named.
///
/// [`DerivedExtent::stands`]: mosaica_store::manifest::DerivedExtent::stands
fn standing_after_retirement(
    store: &ArtifactStore,
    enumerated: &std::collections::BTreeSet<String>,
    entries: impl IntoIterator<Item = (ShardId, mosaica_store::manifest::DerivedExtent)>,
) -> DerivedByShard {
    let mut held = DerivedByShard::new();
    for (shard, entry) in entries {
        let now = store.layer_in(entry.space).and_then(|layer| {
            Some((
                layer,
                store.records_version(layer, entry.level),
                store.slice(shard)?.level_version(layer, entry.level),
            ))
        });
        if let Some((layer, records, slice)) = now {
            if entry.stands(records, slice, enumerated.contains(layer)) {
                held.entry(shard).or_default().push(entry);
                continue;
            }
        }
        tracing::error!(
            space = %entry.space,
            level = entry.level,
            shard = %shard,
            form = entry.form.dir(),
            stamped = ?(entry.level_version, entry.slice_version),
            now = ?now.map(|(_, records, slice)| (records, slice)),
            "ALARM: a derived file a fold publishes does not stand at the versions its level \
             carries after the retirement; it is dropped and the level recomposes on first use"
        );
    }
    held
}

/// Delete every `v#####` tree under the bundle root that `CURRENT` does not name, once, before
/// the executor thread is spawned.
///
/// Must run before the thread starts: once serving begins, a live generation may still hold a
/// prefix `CURRENT` does not name, and [`Executor::pending_reclaim`] reclaims that instead.
pub(in crate::write) fn sweep_orphan_prefixes(bundle_root: &Path, live: &str) {
    let Ok(entries) = std::fs::read_dir(bundle_root) else {
        return;
    };
    let (mut swept, mut refused) = (0usize, 0usize);
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        // A prefix is `v` + five-or-more digits (`next_prefix_name`'s own shape).
        let is_prefix = name
            .strip_prefix('v')
            .is_some_and(|digits| digits.len() >= 5 && digits.bytes().all(|b| b.is_ascii_digit()));
        if !is_prefix || name == live || !entry.path().is_dir() {
            continue;
        }
        match mosaica_store::reclaim_prefix(&entry.path()) {
            Ok(()) => {
                swept += 1;
                tracing::info!(
                    prefix = %name,
                    "startup sweep: reclaimed an orphaned prefix left by a discarded fold or an \
                     exit mid-publication"
                );
            }
            Err(e) => {
                refused += 1;
                tracing::warn!(
                    prefix = %name,
                    error = %e,
                    "startup sweep: could not reclaim an orphaned prefix; it stands, and its disc \
                     with it"
                );
            }
        }
    }
    if swept > 0 || refused > 0 {
        tracing::info!(live = %live, swept, refused, "startup sweep complete");
    }
}

/// What is on disc under `prefix_dir` against what `generation`'s manifests still name, by the
/// shard whose directory holds it. Everything outside every shard's directory counts in `rest`'s.
/// `None` where the tree cannot be walked.
///
/// `named` sums both the bundle-level `MANIFEST.json` and the partition's `SEGMENTS-<n>.json`;
/// the build's own artefacts are in the first and would be missed by reading only the second.
/// `unnamed` is what no manifest names at all: neither map, no derived file and no shard, space or
/// partition file.
fn dead_bytes_of(
    prefix_dir: &Path,
    generation: &Generation,
    rest: ShardId,
) -> Option<std::collections::BTreeMap<ShardId, ShardDeadBytes>> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, u64)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                walk(&entry.path(), base, out);
            } else if let Ok(rel) = entry.path().strip_prefix(base) {
                out.push((rel.to_string_lossy().into_owned(), meta.len()));
            }
        }
    }
    if !prefix_dir.is_dir() {
        return None;
    }
    let mut on_disc = Vec::new();
    walk(prefix_dir, prefix_dir, &mut on_disc);
    let partitions: Vec<String> = generation.bundle.partitions.keys().cloned().collect();
    let owner = |rel: &str| {
        partitions
            .iter()
            .find_map(|partition| mosaica_store::layout::shard_of(partition, rel))
            .unwrap_or(rest)
    };
    let files = generation.bundle.manifest.files.iter().chain(
        generation
            .bundle
            .partitions
            .values()
            .flat_map(|partition| partition.manifest.files.iter()),
    );
    let mut named: FxHashSet<&str> = FxHashSet::default();
    let mut by_shard: std::collections::BTreeMap<ShardId, ShardDeadBytes> = generation
        .shards()
        .map(|shard| (shard.id(), ShardDeadBytes::default()))
        .collect();
    for (rel, digest) in files {
        named.insert(rel.as_str());
        if let Some(gauge) = by_shard.get_mut(&owner(rel)) {
            gauge.dead.named += digest.size;
        }
    }
    for partition in generation.bundle.partitions.values() {
        named.extend(
            partition
                .derived_by_shard()
                .map(|(_, extent)| extent.path.as_str()),
        );
    }
    for (rel, size) in &on_disc {
        let Some(gauge) = by_shard.get_mut(&owner(rel)) else {
            continue;
        };
        gauge.dead.on_disc += size;
        let file = rel.rsplit('/').next().unwrap_or(rel);
        if !mosaica_store::layout::is_manifest_name(file) && !named.contains(rel.as_str()) {
            gauge.unnamed += size;
        }
    }
    Some(by_shard)
}

/// One shard's dead-bytes gauge: the schedule's ratio, and the bytes no manifest names.
#[derive(Debug, Clone, Copy, Default)]
struct ShardDeadBytes {
    dead: crate::compact::DeadBytes,
    unnamed: u64,
}

/// Free bytes on the filesystem holding `path`, or `None` where unknowable.
///
/// Reads `f_bavail`, not `f_bfree`: reserved-for-root blocks are not space a fold may plan to use.
#[cfg(unix)]
pub(super) fn free_disc(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stats) } != 0 {
        return None;
    }
    Some(stats.f_bavail as u64 * stats.f_frsize as u64)
}

#[cfg(not(unix))]
pub(super) fn free_disc(_path: &Path) -> Option<u64> {
    None
}

/// What decides which shard the next compaction is of, beside the schedule: the request being
/// answered, the closing compactions queued at seals, and each shard's last compaction.
#[derive(Default)]
pub(in crate::write) struct Compactions {
    /// The shards a request named and no compaction has been dispatched for, in order. `Some`
    /// until the request is answered: [`ExecutorHealth::fold_requested`] stays raised.
    requested: Option<std::collections::VecDeque<ShardId>>,
    /// The number of the last request a tick took: what a refusal answers, and what
    /// [`ExecutorHealth::compaction_requests_answered`] rises to when `requested` empties.
    requested_through: u64,
    /// The shards sealed since their last compaction, each due one closing compaction, with the
    /// WAL position of the last record the append that sealed it wrote.
    closing: std::collections::BTreeMap<ShardId, u64>,
    /// When each shard's last compaction started, as a unix second, so the interval can limit
    /// attempts.
    started: std::collections::BTreeMap<ShardId, u64>,
    /// When each shard's last compaction ended, as a unix second, but for `last`'s, which the
    /// fold's thread writes to [`ExecutorHealth::fold_ended_unix`].
    ended: std::collections::BTreeMap<ShardId, u64>,
    /// The shard of the compaction dispatched last.
    last: Option<ShardId>,
    /// The thread of the compaction dispatched last.
    thread: Option<CompactionThread>,
    /// The request being answered, from the tick that takes it until it is answered.
    answering: Option<Answering>,
}

/// What a request's answer is made from: the number its range of merged requests begins at, the
/// compaction counters when a tick took it, and the refusal that answers it, where one does. Only
/// the request's compactions run while it is answered, so the counters' rise is theirs.
struct Answering {
    first: u64,
    folds: u64,
    failures: u64,
    refused: Option<RequestOutcome>,
    /// Compactions ended when [`Self::job`] was last brought up to date.
    ended: u64,
    /// The record of the compaction that answers the request: the first that failed where one
    /// did, and the last to end otherwise.
    job: Option<mosaica_types::progress::Progress>,
}

/// A compaction's thread and the flag that abandons it. Dropping it raises the flag and joins the
/// thread, which stops at the end of the pass it is in and removes the prefix it was writing, so
/// the executor's thread ends only once nothing writes under the bundle.
struct CompactionThread {
    abandoned: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for CompactionThread {
    fn drop(&mut self) {
        self.abandoned.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Compactions {
    /// Abandon a running compaction and wait for its thread.
    pub(in crate::write) fn abandon(&mut self) {
        drop(self.thread.take());
    }

    /// Queue a closing compaction for each of `sealed`, by an append that ended at WAL position
    /// `at`.
    pub(in crate::write) fn sealed(&mut self, sealed: impl IntoIterator<Item = ShardId>, at: u64) {
        self.closing
            .extend(sealed.into_iter().map(|shard| (shard, at)));
    }
}

/// Whether `shard` has something a compaction reclaims: a deletion in its overlay, an artifact
/// deletion it must remove, a segment flushed since its last compaction, a column of `runtime` it
/// holds no base of, or bytes under its directory no manifest names. `unnamed` is a directory
/// walk, called only after every other check has declined.
fn reclaims(
    shard: &Shard,
    gauges: &crate::compact::Gauges,
    runtime: &[String],
    unnamed: impl FnOnce() -> u64,
) -> bool {
    gauges.retirable_deletions > 0
        || gauges.artifact_deletions > 0
        || shard
            .data
            .views
            .values()
            .any(|view| !view.row_space.extents().is_empty())
        || !shard.data.record.unbased(runtime).is_empty()
        || unnamed() > 0
}

/// Where a compaction's shard came from.
#[derive(Debug, Clone, Copy)]
enum Dispatching {
    Requested,
    Closing,
    Scheduled(Scheduled),
}

/// Why the schedule called for a compaction of a shard, and the gauges it read.
#[derive(Debug, Clone, Copy)]
struct Scheduled {
    trigger: crate::compact::FoldTrigger,
    gauges: crate::compact::Gauges,
    /// `None` where the live prefix could not be walked.
    dead: Option<ShardDeadBytes>,
}

/// Whether every artefact a completed fold consumed is still listed in the live manifest.
pub(super) fn fold_rebases(
    plan: &crate::compact::FoldPlan,
    live: &mosaica_store::manifest::ShardRecord,
    consumed_segments: &FxHashSet<(&str, &str)>,
) -> bool {
    let listed_segments: FxHashSet<(&str, &str)> = live
        .segments
        .iter()
        .map(|d| (d.view.as_str(), d.seg_id.as_str()))
        .collect();
    consumed_segments.is_subset(&listed_segments)
        && plan.tiers.iter().all(|t| live.deltas.contains(t))
        && plan.attr_extents.iter().all(|consumed| {
            live
                .attr_extents
                .iter()
                .any(|extent| extent.values == consumed.values)
        })
        && plan.record_extents.iter().all(|consumed| {
            live
                .record_extents
                .iter()
                .any(|extent| extent.blocks == consumed.blocks)
        })
        && plan.text_extents.iter().all(|consumed| {
            live
                .text_extents
                .iter()
                .any(|extent| extent.dict == consumed.dict)
        })
        && plan.entity_terms_extents.iter().all(|consumed| {
            live
                .entity_terms_extents
                .iter()
                .any(|extent| extent.terms == consumed.terms)
        })
        // An index removed during the flight takes nothing; one still held must still list every
        // run the fold consumed.
        && {
            let listed: FxHashSet<&str> = live.edited_items.files().collect();
            plan.edited.files().all(|rel| listed.contains(rel))
        }
        && plan.unique.iter().all(|consumed| {
            live
                .unique_indexes
                .iter()
                .find(|index| index.attribute == consumed.attribute)
                .is_none_or(|index| {
                    let listed: FxHashSet<&str> = index.files().collect();
                    consumed.files().all(|rel| listed.contains(rel))
                })
        })
}

/// What [`Executor::fold_still_applies`] found: what the fold carries forward from the live
/// manifest, the deletions it retires, and how many entities the carried extents hold.
struct FoldApplies<'a> {
    forward: CarriedExtents<'a>,
    executed: croaring::Bitmap,
    carried_entities: u64,
}

/// The columns declared at a running service that a compaction writes into `MANIFEST.json`:
/// those every shard holds the base of.
struct Folded {
    entity: Vec<String>,
    scoped: Vec<String>,
}

/// The `(layer, level)`s whose observed memberships overlap, which have no label column.
type Overlapping = std::collections::BTreeSet<(String, u32)>;

/// What every derived writer of one fold shares: where the files go, the row spaces and segments
/// the fold has just written, the levels its retirement is about to move, and the file counter.
struct DerivedPass<'a> {
    prefix_dir: &'a std::path::Path,
    /// The compacted shard's: its files are written and its slice is read.
    layout: ShardLayout<'a>,
    n: u64,
    incarnations: FxHashMap<String, mosaica_types::view::ViewIncarnation>,
    spaces: Vec<(String, mosaica_store::RowSpace)>,
    segments: Vec<(String, Arc<mosaica_store::read::SegmentData>)>,
    pending: &'a PendingRetirement,
    index: mosaica_store::derived::DerivedIndex,
}

/// A level's serving layout, as `(layer, level, layout)`.
type ChosenLayout = (String, u32, mosaica_types::layer::ServingLayout);

/// Each level the store holds, by its space: its layer, its records version and the sum of its
/// slices' versions. Both versions only rise, so a level at the same pair at two readings held the
/// same records and memberships in every shard between them.
type LevelVersions = std::collections::BTreeMap<(LayerSpace, u32), (String, u64, u64)>;

fn level_versions(store: &ArtifactStore) -> LevelVersions {
    store
        .levels_and_extents()
        .filter_map(|(layer, level, _)| {
            let held = (
                layer.to_string(),
                store.records_version(layer, level),
                store.every_slice_version(layer, level),
            );
            Some(((store.space_of(layer)?, level), held))
        })
        .collect()
}

/// What a compaction's thread composes the derived files of its prefix from, read and never
/// written, and the side-manifest number their names carry. The number is spent at the dispatch
/// and names no manifest, so no publication into the prefix names a file the same.
pub(in crate::write) struct DerivedSources {
    live: Arc<LiveState>,
    generation: Arc<GenerationHandle>,
    shapes: Arc<crate::shapes::ShapeStore>,
    projections: Arc<crate::artifacts::ArtifactProjections>,
    n: u64,
}

/// The derived files a compaction's thread wrote, and what they were composed against: the
/// retirement that was due then, each level's versions where no write moved them during the pass,
/// and the registry's version.
struct DerivedWritten {
    own: Vec<mosaica_store::manifest::DerivedExtent>,
    others: Vec<(ShardId, mosaica_store::manifest::DerivedExtent)>,
    layouts: Vec<ChosenLayout>,
    pending: PendingRetirement,
    versions: LevelVersions,
    registry: u64,
}

/// A compaction its thread has finished, ready to publish.
pub(in crate::write) struct WrittenFold {
    completed: crate::compact::CompletedFold,
    /// `None` where the fold no longer applied to the live bundle when its thread looked.
    derived: Option<DerivedWritten>,
}

/// What a compaction of `shard` executing the point deletions `points` retires, with `owing`
/// naming the shards that hold each artifact deletion: each artifact deletion no other shard
/// holds, and each another shard holds released from `shard`'s slice. Beside it, the deletions
/// left settled for the other shards' compactions to remove.
///
/// An artifact has no rows, and an executed one's record, content and members leave the files the
/// compaction writes. One another shard holds stays, denied, until that shard's compaction.
fn retirement(
    live: &Generation,
    owing: impl Fn(
        LayerSpace,
        &croaring::Bitmap,
    ) -> std::collections::BTreeMap<ShardId, croaring::Bitmap>,
    shard: ShardId,
    points: croaring::Bitmap,
) -> (
    mosaica_lifecycle::membership::Retired,
    crate::write::live::SettledSpaces,
) {
    let mut artifacts = std::collections::BTreeMap::new();
    let mut released = std::collections::BTreeMap::new();
    let mut settled = crate::write::live::SettledSpaces::new();
    for (space, parts) in live.layer_spaces.iter() {
        let deleted = parts.overlay.deleted_set();
        if deleted.is_empty() {
            continue;
        }
        let mut owing = owing(*space, deleted);
        let here = owing.remove(&shard).unwrap_or_default();
        let mut still = croaring::Bitmap::new();
        for owed in owing.values() {
            still.or_inplace(owed);
        }
        let gone = deleted.andnot(&still);
        let left = here.andnot(&gone);
        if !left.is_empty() {
            released.insert(*space, left);
        }
        if !gone.is_empty() {
            artifacts.insert(*space, gone);
        }
        if !still.is_empty() {
            settled.insert(
                *space,
                crate::write::live::SettledOwing {
                    deletions: still,
                    by_shard: owing,
                },
            );
        }
    }
    let retired = mosaica_lifecycle::membership::Retired {
        shard,
        points,
        artifacts,
        released,
    };
    (retired, settled)
}

/// The derived files a compaction writes: its own shard's, and every other shard's columns of the
/// levels whose layout it moved, which replace that shard's files of those levels.
struct FoldDerivedFiles {
    own: Vec<mosaica_store::manifest::DerivedExtent>,
    others: Vec<(ShardId, mosaica_store::manifest::DerivedExtent)>,
    moved: Vec<(LayerSpace, u32)>,
}

/// What a fold carries forward: everything the live manifest still lists that the fold did not
/// consume, i.e. what published during its flight.
///
/// Listed order is preserved: for segments it is entity order, required by
/// `RowSpace::with_extent`. Nothing of a
/// dead incarnation is carried; a dropped view's segments are left behind with the superseded
/// prefix's files instead of naming an incarnation the new manifest does not declare.
pub(super) struct CarriedExtents<'a> {
    segments: Vec<&'a mosaica_store::manifest::SegmentDescriptor>,
    tiers: Vec<String>,
    attrs: Vec<mosaica_store::manifest::AttrExtent>,
    records: Vec<mosaica_store::manifest::RecordExtent>,
    texts: Vec<mosaica_store::manifest::TextExtent>,
    entity_terms: Vec<mosaica_store::manifest::EntityTermsExtent>,
    /// Every unique index the live manifest holds, less what the fold consumed: a folded
    /// column's base runs are the fold's own and are left out here.
    unique: Vec<mosaica_store::manifest::UniqueIndexRuns>,
    /// The edited-items live runs written during the fold's flight; the fold's own are its base.
    edited: mosaica_store::manifest::EditedItemsRuns,
    /// The dropped views whose segments were left behind, and how many segments that was.
    omitted_views: Vec<String>,
    omitted_segments: usize,
}

/// The entries of `live` the fold did not consume, in listed order, keyed by file path.
pub(super) fn not_consumed<T: Clone>(
    live: &[T],
    consumed: &FxHashSet<&str>,
    key: impl Fn(&T) -> &str,
) -> Vec<T> {
    live.iter()
        .filter(|entry| !consumed.contains(key(entry)))
        .cloned()
        .collect()
}

pub(super) fn carried_forward<'a>(
    plan: &crate::compact::FoldPlan,
    live: &'a mosaica_store::manifest::ShardRecord,
    live_incarnations: &FxHashMap<&str, mosaica_types::view::ViewIncarnation>,
    consumed_segments: &FxHashSet<(&str, &str)>,
) -> CarriedExtents<'a> {
    let mut omitted_views: Vec<String> = Vec::new();
    let mut omitted_segments = 0usize;
    let segments: Vec<&mosaica_store::manifest::SegmentDescriptor> = live
        .segments
        .iter()
        .filter(|d| !consumed_segments.contains(&(d.view.as_str(), d.seg_id.as_str())))
        .filter(|d| {
            let live = live_incarnations.get(d.view.as_str()) == Some(&d.incarnation);
            if !live {
                omitted_segments += 1;
                if !omitted_views.iter().any(|v| v == &d.view) {
                    omitted_views.push(d.view.clone());
                }
            }
            live
        })
        .collect();
    let mut attrs = not_consumed(
        &live.attr_extents,
        &plan
            .attr_extents
            .iter()
            .map(|extent| extent.values.as_str())
            .collect(),
        |extent| extent.values.as_str(),
    );
    attrs.retain(|extent| {
        crate::filter::carries_live_view(
            &|view| live_incarnations.get(view).copied(),
            extent.view.as_deref(),
            extent.incarnation,
        )
    });
    let mut texts = not_consumed(
        &live.text_extents,
        &plan
            .text_extents
            .iter()
            .map(|extent| extent.dict.as_str())
            .collect(),
        |extent| extent.dict.as_str(),
    );
    texts.retain(|extent| {
        crate::filter::carries_live_view(
            &|view| live_incarnations.get(view).copied(),
            extent.view.as_deref(),
            extent.incarnation,
        )
    });
    CarriedExtents {
        segments,
        tiers: live
            .deltas
            .iter()
            .filter(|t| !plan.tiers.contains(t))
            .cloned()
            .collect(),
        attrs,
        records: not_consumed(
            &live.record_extents,
            &plan
                .record_extents
                .iter()
                .map(|extent| extent.blocks.as_str())
                .collect(),
            |extent| extent.blocks.as_str(),
        ),
        texts,
        entity_terms: not_consumed(
            &live.entity_terms_extents,
            &plan
                .entity_terms_extents
                .iter()
                .map(|extent| extent.terms.as_str())
                .collect(),
            |extent| extent.terms.as_str(),
        ),
        unique: live
            .unique_indexes
            .iter()
            .map(|index| {
                match plan
                    .unique
                    .iter()
                    .find(|folded| folded.attribute == index.attribute)
                {
                    None => index.clone(),
                    Some(folded) => {
                        let consumed: FxHashSet<&str> = folded.files().collect();
                        mosaica_store::manifest::UniqueIndexRuns {
                            attribute: index.attribute.clone(),
                            base: Vec::new(),
                            live: index
                                .live
                                .iter()
                                .filter(|rel| !consumed.contains(rel.as_str()))
                                .cloned()
                                .collect(),
                        }
                    }
                }
            })
            .collect(),
        edited: {
            let consumed: FxHashSet<&str> = plan.edited.files().collect();
            let unconsumed =
                |runs: &mosaica_store::manifest::KeyRuns| mosaica_store::manifest::KeyRuns {
                    base: Vec::new(),
                    live: runs
                        .live
                        .iter()
                        .filter(|rel| !consumed.contains(rel.as_str()))
                        .cloned()
                        .collect(),
                };
            mosaica_store::manifest::EditedItemsRuns {
                by_number: unconsumed(&live.edited_items.by_number),
                by_entity: unconsumed(&live.edited_items.by_entity),
            }
        },
        omitted_views,
        omitted_segments,
    }
}

/// Exactly the files the new manifest names that the fold did not write, deduplicated: linking one
/// path twice is what `hard_link_forward` refuses.
pub(super) fn carried_files(
    layout: ShardLayout<'_>,
    live_manifest: &mosaica_store::manifest::PartitionManifest,
    bundle_files: &std::collections::BTreeMap<String, mosaica_store::manifest::FileDigest>,
    forward: &CarriedExtents,
    shared_tenancy: &[mosaica_store::manifest::TenancyBit],
) -> std::collections::BTreeSet<String> {
    // A segment an earlier fold carried has its files listed in the bundle's manifest.
    let listed =
        |rel: &str| live_manifest.files.contains_key(rel) || bundle_files.contains_key(rel);
    let mut rels: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for descriptor in &forward.segments {
        let segment_prefix = layout.segment_rel(&descriptor.view, &descriptor.seg_id);
        for name in mosaica_store::SEGMENT_FILES {
            rels.insert(format!("{segment_prefix}/{name}"));
        }
        let presence_prefix = format!("{segment_prefix}/{}/", RENDER_PRESENCE_DIR);
        rels.extend(
            live_manifest
                .files
                .keys()
                .chain(bundle_files.keys())
                .filter(|rel| rel.starts_with(&presence_prefix))
                .cloned(),
        );
        let edited_rows = format!(
            "{segment_prefix}/{}",
            mosaica_store::edited::EDITED_ROWS_FILE
        );
        if listed(&edited_rows) {
            rels.insert(edited_rows);
        }
    }
    rels.extend(forward.tiers.iter().cloned());
    for extent in &forward.attrs {
        rels.extend(extent.files().map(String::from));
    }
    for extent in &forward.records {
        rels.extend(extent.files().map(String::from));
    }
    for extent in &forward.texts {
        rels.extend(extent.files().map(String::from));
    }
    for extent in &forward.entity_terms {
        rels.extend(extent.files().map(String::from));
    }
    for index in &forward.unique {
        rels.extend(index.files().map(String::from));
    }
    rels.extend(forward.edited.files().map(String::from));
    rels.extend(live_manifest.dict_extents.iter().map(|e| e.path.clone()));
    rels.extend(shared_tenancy.iter().map(|bit| bit.path.clone()));
    rels
}

/// What a compaction of `shard` carries of every other shard of `partition`: each entry of the
/// live `MANIFEST.json`'s and partition manifest's `files` under another shard's directory, less
/// the member packs, which the compaction writes again. The new prefix links each, and lists it in
/// the same map it came from.
struct OtherShards {
    bundle: std::collections::BTreeMap<String, mosaica_store::manifest::FileDigest>,
    side: std::collections::BTreeMap<String, mosaica_store::manifest::FileDigest>,
}

fn other_shards(
    partition: &str,
    shard: ShardId,
    live_manifest: &mosaica_store::manifest::PartitionManifest,
    bundle_files: &std::collections::BTreeMap<String, mosaica_store::manifest::FileDigest>,
) -> OtherShards {
    let carried =
        |files: &std::collections::BTreeMap<String, mosaica_store::manifest::FileDigest>| {
            files
                .iter()
                .filter(|(rel, _)| {
                    mosaica_store::layout::shard_of(partition, rel).is_some_and(|at| at != shard)
                        && !mosaica_store::layout::is_artifact_file(partition, rel)
                })
                .map(|(rel, digest)| (rel.clone(), digest.clone()))
                .collect()
        };
    OtherShards {
        bundle: carried(bundle_files),
        side: carried(&live_manifest.files),
    }
}

/// The derived files a compaction of `shard` keeps from `held`: every other shard's, and a layer
/// space's held shapes of a view and level the compaction did not write again. Its own are the
/// ones it writes.
fn kept_derived(
    held: &DerivedByShard,
    shard: ShardId,
    written: &[mosaica_store::manifest::DerivedExtent],
) -> Vec<(ShardId, mosaica_store::manifest::DerivedExtent)> {
    use mosaica_store::manifest::DerivedForm;
    let rewritten = |extent: &mosaica_store::manifest::DerivedExtent| {
        written.iter().any(|ours| {
            ours.form == DerivedForm::ShapeHeld
                && ours.space == extent.space
                && ours.level == extent.level
                && ours.view == extent.view
        })
    };
    held.iter()
        .flat_map(|(held, extents)| extents.iter().map(move |extent| (*held, extent)))
        .filter(|(held, extent)| match extent.form {
            DerivedForm::ShapeHeld => !rewritten(extent),
            _ => *held != shard,
        })
        .map(|(held, extent)| (held, extent.clone()))
        .collect()
}

/// A level's shape over `row_count` rows from `walk`, and the ordinals holding a row there.
fn observe_held(
    row_count: u32,
    walk: mosaica_store::derived::LevelWalk<'_>,
) -> (crate::layout::LevelShape, croaring::Bitmap) {
    let held = std::cell::RefCell::new(croaring::Bitmap::new());
    let shape = mosaica_store::derived::observe_shape(row_count, &|visit| {
        walk(&mut |ordinal, rows| {
            if !rows.is_empty() {
                held.borrow_mut().add(ordinal);
            }
            visit(ordinal, rows);
        })
    });
    (shape, held.into_inner())
}

/// The members of each artifact of `layer`'s `level` that `shard`'s slice of `store` holds once
/// `pending` has run, by ordinal: copied, so a derived writer reads them under the store's lock and
/// projects them once it is released.
fn held_members(
    store: &ArtifactStore,
    pending: &PendingRetirement,
    shard: ShardId,
    layer: &str,
    level: u32,
) -> Vec<(u32, mosaica_lifecycle::membership::Members)> {
    let slice = slice_of(store, shard);
    pending
        .held(store, shard, layer, level)
        .map(|(ordinal, record)| (ordinal, slice.members_of(record).clone()))
        .collect()
}

/// Whether `extent` is one of the files a level holds of its layout, which a compaction that moves
/// the level's layout writes again in every shard.
fn superseded_by_a_move(
    extent: &mosaica_store::manifest::DerivedExtent,
    moved: &[(LayerSpace, u32)],
) -> bool {
    use mosaica_store::manifest::DerivedForm;
    moved.contains(&(extent.space, extent.level))
        && matches!(
            extent.form,
            DerivedForm::RowColumn { .. }
                | DerivedForm::RowMembers
                | DerivedForm::LevelLabels
                | DerivedForm::BandLabels { .. }
        )
}

/// Whether a completed fold still applies to the live bundle, asked before anything is written:
/// what it carries forward from the live manifest and the deletions it retires, or why it is
/// discarded.
fn fold_applies<'a>(
    completed: &crate::compact::CompletedFold,
    live: &'a Generation,
) -> Result<FoldApplies<'a>, String> {
    let plan = &completed.plan;
    if live.prefix != plan.prefix {
        return Err("it was planned against a superseded prefix".to_string());
    }
    let Some(partition_data) = live.bundle.partitions.get(&plan.partition) else {
        return Err("the live bundle no longer carries the partition it folded".to_string());
    };
    let Some(live_shard) = partition_data.shard(plan.shard) else {
        return Err("the live bundle no longer carries the shard it compacted".to_string());
    };
    let live_record = &live_shard.record;

    // ---- rebase or discard ------------------------------------------------------------------
    //
    // ABA-safe: ids are never reused, so an artefact still listed is the artefact the fold
    // consumed.
    let consumed_segments: FxHashSet<(&str, &str)> = plan
        .views
        .iter()
        .flat_map(|view| {
            view.segments
                .iter()
                .map(move |segment| (view.view.as_str(), segment.seg_id.as_str()))
        })
        .collect();
    if !fold_rebases(plan, live_record, &consumed_segments) {
        return Err("an artefact it consumed is no longer listed in the live manifest".to_string());
    }

    // ---- the carry-forward set ----------------------------------------------------------------
    //
    // Which incarnations are live is decided from the `MANIFEST.json` roster, not the
    // partition's view map (a cache of open row spaces), so it cannot disagree with what the
    // new manifest declares.
    let live_incarnations: FxHashMap<&str, mosaica_types::view::ViewIncarnation> = live
        .bundle
        .manifest
        .views
        .iter()
        .map(|v| (v.id.as_str(), v.incarnation))
        .collect();
    // A view dropped during the flight, or dropped and created again, would have its dead
    // incarnation's base published as the live view's rows. The next fold plans the live one.
    if plan
        .views
        .iter()
        .any(|view| live_incarnations.get(view.view.as_str()) != Some(&view.incarnation))
    {
        return Err(
            "a view it folded was dropped during its flight; the next fold plans over \
                 the views as they now stand"
                .to_string(),
        );
    }
    let forward = carried_forward(plan, live_record, &live_incarnations, &consumed_segments);

    // Every carried-forward segment must begin at or above its view's base, or the prefix will
    // not open. Checked before anything is written, since that would surface only after
    // `CURRENT` flips.
    for descriptor in &forward.segments {
        let Some(view) = plan.views.iter().find(|s| s.view == descriptor.view) else {
            // A view created and flushed since the plan was taken has no base here; that is
            // transient, and the next fold plans over a bundle that holds it.
            return Err(
                "a carried-forward segment names a view created since the plan was taken, so \
                 the fold has no base for it; the next fold plans over a bundle that has it"
                    .to_string(),
            );
        };
        if descriptor.entity_lo < view.permutation_bound {
            return Err(
                "a carried-forward segment begins below the fold's own base permutation"
                    .to_string(),
            );
        }
    }
    // ---- retirement, evaluated here and nowhere earlier ---------------------------------------
    let mut carried = crate::compact::CarriedForward::new();
    for descriptor in &forward.segments {
        let Some(extent) = live_shard.views.get(&descriptor.view).and_then(|view| {
            view.row_space
                .extents()
                .iter()
                .find(|extent| extent.seg_id == descriptor.seg_id)
        }) else {
            return Err(
                "a carried-forward segment has no extent in the live row space".to_string(),
            );
        };
        carried.add_segment(extent);
    }
    let executed = crate::compact::executed(&plan.tombstones, &carried);
    Ok(FoldApplies {
        carried_entities: carried.len(),
        forward,
        executed,
    })
}

/// Every `(layer, level)` the artifact store holds.
pub(super) fn levels_of(store: &ArtifactStore) -> impl Iterator<Item = (String, u32)> + '_ {
    store
        .levels_and_extents()
        .map(|(layer, level, _)| (layer.to_string(), level))
}

impl Executor {
    /// Each shard's gauges, as the schedule reads them. An artifact deletion counts in each shard
    /// that holds it, and one no shard holds in the last shard's (`LiveState::last_shard`).
    fn shard_gauges(&self, generation: &Generation) -> Vec<(ShardId, crate::compact::Gauges)> {
        let mut artifact_deletions: std::collections::BTreeMap<ShardId, u64> = Default::default();
        let last = self.live.last_shard().id;
        let mut shards = generation.shards();
        let several = match (shards.next(), shards.next()) {
            (Some(only), None) => {
                // Every artifact deletion is the one shard's, held by its slice or not: no slice
                // is probed.
                artifact_deletions.insert(only.id(), generation.space_overlay_figures().1);
                false
            }
            _ => true,
        };
        for (space, parts) in generation.layer_spaces.iter().filter(|_| several) {
            let deleted = parts.overlay.deleted_set();
            if deleted.is_empty() {
                continue;
            }
            let mut held_somewhere = croaring::Bitmap::new();
            for (shard, artifacts) in self.live.owing(*space, deleted) {
                *artifact_deletions.entry(shard).or_default() += artifacts.cardinality();
                held_somewhere.or_inplace(&artifacts);
            }
            *artifact_deletions.entry(last).or_default() +=
                deleted.andnot_cardinality(&held_somewhere);
        }
        generation
            .shards()
            .map(|shard| {
                let gauges = crate::compact::Gauges {
                    live_segments: crate::status::live_segments_in(shard),
                    retirable_deletions: shard.overlay.deleted_len(),
                    artifact_deletions: artifact_deletions.get(&shard.id()).copied().unwrap_or(0),
                    live_rows: crate::status::live_rows_in(shard),
                };
                (shard.id(), gauges)
            })
            .collect()
    }

    /// Each shard's dead-bytes gauge, from one walk of the live prefix.
    fn dead_bytes(
        &self,
        generation: &Generation,
    ) -> Option<std::collections::BTreeMap<ShardId, ShardDeadBytes>> {
        dead_bytes_of(
            &self.prefix_dir(generation),
            generation,
            self.live.last_shard().id,
        )
    }

    /// Each shard's gauges, and whether it has something a compaction reclaims ([`reclaims`]).
    fn reclaiming(&self, generation: &Generation) -> Vec<(ShardId, crate::compact::Gauges, bool)> {
        let dead = std::cell::OnceCell::new();
        let unnamed = |shard: ShardId| {
            dead.get_or_init(|| self.dead_bytes(generation).unwrap_or_default())
                .get(&shard)
                .map_or(0, |gauge| gauge.unnamed)
        };
        let (attributes, scoped) = self.live.attributes_for_publication();
        let runtime: Vec<String> = attributes
            .into_iter()
            .map(|declared| declared.name)
            .chain(scoped.into_iter().map(|family| family.name))
            .collect();
        self.shard_gauges(generation)
            .into_iter()
            .map(|(shard, gauges)| {
                let reclaims = generation
                    .shard(shard)
                    .is_some_and(|held| reclaims(held, &gauges, &runtime, || unnamed(shard)));
                (shard, gauges, reclaims)
            })
            .collect()
    }

    /// Which shard [`crate::compact::CompactionSchedule`] calls for a compaction of now, and why:
    /// among the shards due on their own gauges and interval, the one [`crate::compact::rank`] puts
    /// first.
    ///
    /// The dead-bytes gauge is a directory walk and is passed as a closure: `due` calls it only
    /// after every cheaper gauge has declined, and the walk is taken at most once and shared by
    /// every shard. It walks the live prefix, not the bundle root; an orphaned prefix is the
    /// startup sweep's to reclaim, not a fold's.
    fn scheduled_fold(&self, generation: &Arc<Generation>) -> Option<(ShardId, Scheduled)> {
        let now = unix_now()?;
        // No shard past its floor is due on any gauge, so none is read.
        if !generation.shards().any(|shard| {
            crate::compact::past_floor(&self.deps.compaction, now, self.fold_floor_from(shard.id()))
        }) {
            return None;
        }
        let dead = std::cell::OnceCell::new();
        let walked = || dead.get_or_init(|| self.dead_bytes(generation));
        let (shard, trigger, gauges) = self
            .shard_gauges(generation)
            .into_iter()
            .filter_map(|(shard, gauges)| {
                let trigger = crate::compact::due(
                    &self.deps.compaction,
                    now,
                    self.fold_floor_from(shard),
                    gauges,
                    || walked().as_ref()?.get(&shard).map(|gauge| gauge.dead),
                )?;
                Some((crate::compact::rank(shard, &gauges), shard, trigger, gauges))
            })
            .min_by_key(|(key, ..)| *key)
            .map(|(_, shard, trigger, gauges)| (shard, trigger, gauges))?;
        let dead = walked()
            .as_ref()
            .and_then(|by_shard| by_shard.get(&shard).copied());
        Some((
            shard,
            Scheduled {
                trigger,
                gauges,
                dead,
            },
        ))
    }

    /// The shards a request names, in the order they are compacted: those it names; or, for
    /// every shard, those with something to reclaim, ascending, and where none has, the one
    /// [`crate::compact::rank`] puts first.
    fn requested_shards(
        &self,
        generation: &Generation,
        request: crate::write::health::CompactionRequest,
    ) -> std::collections::VecDeque<ShardId> {
        let held: std::collections::BTreeSet<ShardId> =
            generation.shards().map(|shard| shard.id()).collect();
        let mut shards: std::collections::BTreeSet<ShardId> =
            request.shards.intersection(&held).copied().collect();
        if request.every {
            let reclaiming = self.reclaiming(generation);
            match reclaiming.iter().any(|(_, _, reclaims)| *reclaims) {
                true => shards.extend(
                    reclaiming
                        .iter()
                        .filter(|(_, _, reclaims)| *reclaims)
                        .map(|(shard, ..)| *shard),
                ),
                false => shards.extend(
                    reclaiming
                        .iter()
                        .min_by_key(|(shard, gauges, _)| crate::compact::rank(*shard, gauges))
                        .map(|(shard, ..)| *shard),
                ),
            }
        }
        shards.into_iter().collect()
    }

    /// The instant `compaction_min_interval_secs` is measured from for `shard`: a compaction of it
    /// may start no sooner than the interval after its previous one started, and never before
    /// that one has ended. Measuring from the end alone would drift the schedule by the
    /// compaction's own duration; measuring from the start alone would let one that discards
    /// re-dispatch immediately.
    fn fold_floor_from(&self, shard: ShardId) -> Option<u64> {
        let started = *self.compactions.started.get(&shard)?;
        let ended = match self.compactions.last == Some(shard) {
            true => self.health.fold_ended_unix.load(Ordering::SeqCst),
            false => self.compactions.ended.get(&shard).copied().unwrap_or(0),
        };
        Some(started.max(ended.saturating_sub(self.deps.compaction.min_interval_secs)))
    }

    /// Answer the request being worked through where no shard of it is left to dispatch: the
    /// caller knows none is in flight. [`ExecutorHealth::fold_requested`] stays raised while a
    /// request no tick has taken is held.
    fn request_answered(&mut self) {
        if self
            .compactions
            .requested
            .as_ref()
            .is_some_and(|left| left.is_empty())
        {
            self.compactions.requested = None;
        }
        self.note_compaction_ended();
        if self.compactions.requested.is_none() {
            if let Some(answering) = self.compactions.answering.take() {
                let failures = self.health.fold_failures.load(Ordering::SeqCst);
                let folds = self.health.folds.load(Ordering::SeqCst);
                let outcome = match answering.refused {
                    Some(refused) => refused,
                    None if failures > answering.failures => RequestOutcome::Failed,
                    None if folds > answering.folds => RequestOutcome::Compacted,
                    None => RequestOutcome::NothingToDo,
                };
                self.health.keep_answer(RequestAnswer {
                    first: answering.first,
                    last: self.compactions.requested_through,
                    outcome,
                    job: answering.job,
                });
                self.health
                    .compaction_serving_through
                    .store(0, Ordering::SeqCst);
            }
            self.health
                .compaction_requests_answered
                .fetch_max(self.compactions.requested_through, Ordering::SeqCst);
            // Under the lock a request is raised under, so a request raised now is not lowered.
            let held = lock_recover(&self.health.compaction_request);
            if held.is_none() {
                self.health.fold_requested.store(false, Ordering::SeqCst);
            }
        }
    }

    /// The shard the next compaction is of, and why: the request being answered, then a closing
    /// compaction, then the schedule. `None` where nothing calls for one.
    fn next_compaction(&mut self, generation: &Arc<Generation>) -> Option<(ShardId, Dispatching)> {
        if let Some(shard) = self
            .compactions
            .requested
            .as_ref()
            .and_then(|left| left.front())
        {
            return Some((*shard, Dispatching::Requested));
        }
        if !self.compactions.closing.is_empty() {
            let reclaiming = self.reclaiming(generation);
            for (shard, sealed_at) in self.compactions.closing.clone() {
                // Rows the shard holds from the sealing append or before it are flushed first, so
                // its closing compaction reclaims them. Its edits' rows since need not wait.
                let unflushed = generation.shard(shard).is_some_and(|held| {
                    held.buffer
                        .oldest_wal_pos()
                        .is_some_and(|oldest| oldest.is_none_or(|at| at <= sealed_at))
                });
                if unflushed {
                    continue;
                }
                let reclaims = reclaiming
                    .iter()
                    .any(|(at, _, reclaims)| *at == shard && *reclaims);
                if reclaims {
                    return Some((shard, Dispatching::Closing));
                }
                // A sealed shard with nothing to reclaim is due nothing.
                self.compactions.closing.remove(&shard);
            }
        }
        self.scheduled_fold(generation)
            .map(|(shard, scheduled)| (shard, Dispatching::Scheduled(scheduled)))
    }

    /// `shard`'s compaction is dispatched or refused: it leaves the request or the closing queue
    /// it came from.
    fn compaction_taken(&mut self, shard: ShardId, from: Dispatching) {
        match from {
            Dispatching::Requested => {
                if let Some(left) = &mut self.compactions.requested {
                    left.retain(|held| *held != shard);
                }
            }
            Dispatching::Closing => {
                self.compactions.closing.remove(&shard);
            }
            Dispatching::Scheduled(_) => {}
        }
    }

    /// Take the request numbered `request`, merged into the one being answered where there is one.
    fn take_request(&mut self, request: u64) {
        self.compactions.requested_through = request;
        self.health
            .compaction_serving_through
            .store(request, Ordering::SeqCst);
        if self.compactions.answering.is_none() {
            let folds = self.health.folds.load(Ordering::SeqCst);
            let failures = self.health.fold_failures.load(Ordering::SeqCst);
            self.compactions.answering = Some(Answering {
                first: self
                    .health
                    .compaction_requests_answered
                    .load(Ordering::SeqCst)
                    + 1,
                folds,
                failures,
                refused: None,
                ended: folds + failures,
                job: None,
            });
        }
    }

    /// Keep the record of a compaction of the request being answered that has ended since the
    /// last look. Called with no compaction in flight, before the next is dispatched and when the
    /// request is answered, so the last record to end is that compaction's. A failed record is
    /// kept over any later one.
    fn note_compaction_ended(&mut self) {
        if self.fold.outstanding() {
            return;
        }
        let Some(answering) = &mut self.compactions.answering else {
            return;
        };
        let ended = self.health.folds.load(Ordering::SeqCst)
            + self.health.fold_failures.load(Ordering::SeqCst);
        if ended == answering.ended {
            return;
        }
        answering.ended = ended;
        let failed = |job: &Option<mosaica_types::progress::Progress>| {
            job.as_ref()
                .is_some_and(|job| job.state == mosaica_types::progress::JobState::Failed)
        };
        if !failed(&answering.job) {
            answering.job = self
                .health
                .jobs
                .last_ended(mosaica_types::progress::JobKind::Compaction);
        }
    }

    /// Record that `reason` refused `shard` of the request being answered, or the whole request
    /// where `shard` is `None`. A shard with nothing to compact refuses nothing the request asked
    /// for.
    fn refuse_requested(&mut self, reason: crate::compact::NoFold, shard: Option<ShardId>) {
        if reason == crate::compact::NoFold::NothingToFold {
            return;
        }
        if let Some(answering) = &mut self.compactions.answering {
            let (gate, need_bytes, had_bytes) = reason.gauge();
            answering.refused.get_or_insert(RequestOutcome::Refused {
                gate,
                detail: reason.detail(),
                shard,
                need_bytes,
                had_bytes,
            });
        }
    }

    /// Answer the request held or being worked through with `reason`, a state that stops this node
    /// publishing until a restart.
    fn refuse_request_unpublishable(&mut self, reason: crate::compact::NoFold) {
        if let Some(request) = self.health.take_compaction_request() {
            self.take_request(request.request);
        } else if self.compactions.requested.is_none() {
            return;
        }
        self.health.record_fold_refusal(reason);
        self.refuse_requested(reason, None);
        self.compactions.requested = None;
        self.request_answered();
        tracing::warn!(
            gate = ?reason,
            "a compaction was requested and refused: this node publishes nothing until restarted"
        );
    }

    /// Plan a compaction and start it on its own thread, if one is requested, due or called for
    /// by a seal and nothing blocks it. Not on the shared rayon pool: a compaction's input is a
    /// shard of the corpus, and occupying request-serving workers for its duration would let
    /// maintenance compete with the product it serves.
    ///
    /// A refusal (a latched gate) consumes the shard's place in a request. A request that arrives
    /// while a compaction, a merge, a coalesce or a unique declaration's build is outstanding waits
    /// for a later tick (see [`Executor::fold_outstanding`]).
    pub(super) fn dispatch_fold(&mut self, generation: &Arc<Generation>) {
        // A compaction this node could not publish is hours of IO spent to produce an orphan. The
        // planner's own gates cover the recoverable postures; this one covers the two that latch,
        // and answers a request with a refusal, since neither clears before a restart.
        if let Some(reason) = self.health.unpublishable() {
            self.refuse_request_unpublishable(reason);
            return;
        }
        // At most one compaction is in flight, and a completed one not yet drained is one pass
        // from publication. A request stays held, `fold_requested` raised, and the first tick
        // after the landing takes it.
        if self.fold_outstanding() {
            return;
        }
        // None while a merge, a coalesce or a unique declaration's build is outstanding. A merge's
        // or a coalesce's output would be orphaned by the flip and its inputs are the
        // compaction's, so the rebase check would discard the compaction. A build's rounds read
        // the layers and runs a compaction rewrites. Each writes files no manifest names until it
        // publishes, so a request is resolved, and a closing compaction's shard tested, only once
        // none is outstanding.
        if self.merge_outstanding()
            || self.coalesce_outstanding()
            || self.unique_declarations.busy()
        {
            return;
        }
        // A request is resolved to its shards at this tick, against the gauges as they stand.
        if let Some(request) = self.health.take_compaction_request() {
            self.take_request(request.request);
            let shards = self.requested_shards(generation, request);
            match &mut self.compactions.requested {
                Some(left) => {
                    for shard in shards {
                        if !left.contains(&shard) {
                            left.push_back(shard);
                        }
                    }
                }
                None => self.compactions.requested = Some(shards),
            }
        }
        self.request_answered();
        // A request or a closing compaction dispatches on its own terms, whatever hour it is and
        // whatever the gauges read, so the schedule is not consulted for one.
        let Some((shard_id, from)) = self.next_compaction(generation) else {
            return;
        };

        let plan = match crate::compact::plan_fold(
            generation,
            shard_id,
            self.log.wal.is_poisoned(),
            self.health.overlay_diverged.load(Ordering::SeqCst),
            crate::compact::FoldResources {
                available_memory: match self.deps.switches.fold_memory.load(Ordering::SeqCst) {
                    0 => available_memory(),
                    bytes => Some(bytes),
                },
                free_disc: free_disc(&self.deps.bundle_root),
                membership_containers: self.live.with_artifacts(|store| {
                    store
                        .slice(shard_id)
                        .map_or(0, |slice| slice.membership_containers())
                }),
            },
        ) {
            Ok(plan) => plan,
            Err(reason) => {
                // Recorded before the request is answered, so a caller that reads it answered
                // finds the refusal.
                self.health.record_fold_refusal(reason);
                if matches!(from, Dispatching::Requested) {
                    self.refuse_requested(reason, Some(shard_id));
                }
                self.compaction_taken(shard_id, from);
                self.request_answered();
                tracing::warn!(
                    gate = ?reason,
                    shard = %shard_id,
                    "a compaction was called for and refused: this node compacts nothing of the \
                     shard in this state, and the call is answered rather than retried"
                );
                return;
            }
        };

        let manifest = &generation.bundle.manifest;
        // One schema per view: the bundle-wide render tail plus that view's group-scoped lanes.
        let scalar_schema: std::collections::BTreeMap<String, Vec<(String, ScalarType)>> = plan
            .views
            .iter()
            .map(|view| {
                (
                    view.view.clone(),
                    view_scalar_schema_of(manifest, &view.view),
                )
            })
            .collect();
        let Some(partition_data) = generation.bundle.partitions.get(&plan.partition) else {
            return;
        };
        // The columns declared at a running service and not yet folded, taken at the plan so a
        // declaration made while the fold runs is not among them.
        let (runtime_attributes, runtime_scoped_attributes) = {
            let (entity, scoped) = self.live.attributes_for_publication();
            (
                entity.into_iter().map(|d| d.name).collect::<Vec<_>>(),
                scoped.into_iter().map(|f| f.name).collect::<Vec<_>>(),
            )
        };
        // Per view, the columns an input segment may lawfully lack: the runtime columns above and
        // the view's group-scoped lanes. Read from `runtime_attributes`, not a second snapshot of
        // the live list, so the two cannot disagree about a declaration landing between them.
        let absent_ok: std::collections::BTreeMap<String, Vec<String>> = {
            let entity_scoped = scalar_schema_of(manifest).len();
            scalar_schema
                .iter()
                .map(|(view, schema)| {
                    (
                        view.clone(),
                        lawful_absences(schema, entity_scoped, &runtime_attributes),
                    )
                })
                .collect()
        };
        // The compaction is in flight, and its record listed, from here, so a failure to name its
        // prefix is the record's failure too.
        let rows = plan.views.iter().map(|view| view.rows).sum();
        let stairs = crate::compact::Staircase::start(shard_id, rows);
        let unit = self
            .fold
            .start_recorded(&self.health.jobs, Arc::clone(stairs.record()));
        let named = crate::compact::next_prefix_name(&self.deps.bundle_root)
            .map_err(|e| format!("could not name a new prefix for the fold ({e})"))
            .and_then(|prefix| {
                let n = self
                    .side_manifests
                    .allocate_manifest_n(&self.deps.bundle_root, &self.health)
                    .map_err(|e| {
                        format!("could not take a number for the fold's derived files ({e})")
                    })?;
                Ok((prefix, n))
            });
        let (to_prefix, derived_n) = match named {
            Ok(named) => named,
            Err(e) => {
                // Counted before the record ends and the request is answered, so the answer is a
                // failure.
                self.health.fold_failures.fetch_add(1, Ordering::Relaxed);
                drop(unit);
                self.compaction_taken(shard_id, from);
                self.request_answered();
                tracing::error!("{e}");
                return;
            }
        };
        let sources = DerivedSources {
            live: Arc::clone(&self.live),
            generation: Arc::clone(&self.generation),
            shapes: Arc::clone(&self.deps.shapes),
            projections: Arc::clone(&self.deps.artifact_projections),
            n: derived_n,
        };

        let attempt = self.fold.next_attempt();
        let shard = generation.shard_of(shard_id);
        let unbased = shard.data.record.unbased(&runtime_attributes);
        let scoped_scalars = manifest.scoped_scalars();
        let mut scoped_views = Vec::new();
        let mut unbased_scoped = Vec::new();
        for family in &scoped_scalars {
            for view in &family.views {
                let Some(incarnation) = manifest.incarnation_of(view) else {
                    continue;
                };
                let column = mosaica_store::manifest::ScopedColumn {
                    column: family.name.clone(),
                    view: view.clone(),
                    incarnation,
                };
                if !partition_data.scoped_base_in(shard_id, family, view, incarnation) {
                    unbased_scoped.push(column.clone());
                }
                scoped_views.push(column);
            }
        }
        let ctx = crate::compact::FoldContext {
            from_prefix_dir: self.prefix_dir(generation),
            to_prefix_dir: self.deps.bundle_root.join(&to_prefix),
            to_prefix: to_prefix.clone(),
            identity_key: self.deps.identity_key,
            scalar_schema,
            band_schema: band_schema_of(manifest),
            absent_ok,
            runtime_attributes,
            runtime_scoped_attributes,
            unbased,
            unbased_scoped,
            scoped_views,
            // A `seg_id` must never be reused across the bundle's life, or a rebase's ABA check
            // would be meaningless.
            seg_id: format!("fold-{}-{attempt}", partition_data.segments_n),
            base_postings: Arc::clone(&shard.postings),
            tiers: shard.delta_postings.clone(),
            declared_scalars: manifest.declared_scalars.clone(),
            scoped_scalars,
            vocabularies: manifest.vocabularies.clone(),
            abandoned: Arc::new(AtomicBool::new(false)),
        };

        if let Dispatching::Scheduled(Scheduled {
            trigger,
            gauges,
            dead,
        }) = from
        {
            // Every gauge, not only the one that fired: a compaction is minutes to hours, and an
            // operator reading why one started needs to see the state that produced it, not just
            // the single number that crossed first. The dead-bytes pair especially, since it is
            // the one figure `/control/status` does not carry.
            tracing::info!(
                trigger = ?trigger,
                shard = %shard_id,
                live_segments = gauges.live_segments,
                retirable_deletions = gauges.retirable_deletions,
                artifact_deletions = gauges.artifact_deletions,
                live_rows = gauges.live_rows,
                on_disc_bytes = dead.map(|d| d.dead.on_disc),
                named_bytes = dead.map(|d| d.dead.named),
                "dispatching a scheduled compaction of a shard"
            );
        }
        self.compaction_taken(shard_id, from);
        self.health
            .compaction_shard
            .store(shard_id.raw(), Ordering::SeqCst);
        let health = Arc::clone(&self.health);
        let switches = Arc::clone(&self.deps.switches);
        let abandoned = Arc::clone(&ctx.abandoned);
        let spawned = std::thread::Builder::new()
            .name("mosaica-fold".to_string())
            .spawn(move || {
                let abandoned = Arc::clone(&ctx.abandoned);
                let to_prefix_dir = ctx.to_prefix_dir.clone();
                // A pass that panics fails the compaction as an error does.
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Test hook; always false otherwise. See `Engine::panic_next_fold_for_test`.
                    if switches.fold_panics.swap(false, Ordering::SeqCst) {
                        panic!("the test hook made the compaction panic");
                    }
                    let mut completed = crate::compact::execute(plan, ctx, stairs)?;
                    if abandoned.load(Ordering::SeqCst) {
                        return Err(crate::flush::MaintenanceFailed(
                            "the compaction was abandoned because the engine closed".to_string(),
                        ));
                    }
                    // Here rather than at the publication, which runs on the thread that answers
                    // every write: the derived files are read from the whole shard.
                    completed.stairs.enter(crate::compact::Pass::Derived, None);
                    let derived = sources.write(&completed, &to_prefix_dir);
                    Ok(WrittenFold { completed, derived })
                }))
                .unwrap_or_else(|panic| {
                    Err(crate::flush::MaintenanceFailed(format!(
                        "a pass of the compaction panicked: {}",
                        crate::engine::panic_message(panic.as_ref())
                    )))
                });
                // Test hook; always false otherwise. See `Engine::set_fold_paused_for_test`.
                if outcome.is_ok() {
                    health.fold_holding.store(true, Ordering::SeqCst);
                    while switches.fold_paused.load(Ordering::SeqCst)
                        && !abandoned.load(Ordering::SeqCst)
                    {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    health.fold_holding.store(false, Ordering::SeqCst);
                }
                if abandoned.load(Ordering::SeqCst) {
                    if let Err(e) = mosaica_store::reclaim_prefix(&to_prefix_dir) {
                        tracing::warn!(
                            error = %e,
                            prefix = %to_prefix_dir.display(),
                            "an abandoned compaction's prefix could not be removed; the next \
                             executor's startup sweep removes it"
                        );
                    }
                    return;
                }
                match outcome {
                    Ok(mut written) => {
                        written
                            .completed
                            .stairs
                            .enter(crate::compact::Pass::HandOff, None);
                        // Before the in-flight flag clears, so a tick that observes the fold
                        // finished also observes when. See `fold_floor_from`.
                        health
                            .fold_ended_unix
                            .store(unix_now().unwrap_or(0), Ordering::SeqCst);
                        unit.complete(written);
                    }
                    Err(e) => {
                        health
                            .fold_ended_unix
                            .store(unix_now().unwrap_or(0), Ordering::SeqCst);
                        // Counted before the flag clears, so a caller that sees its request
                        // answered sees the failure.
                        health.fold_failures.fetch_add(1, Ordering::Relaxed);
                        drop(unit);
                        tracing::warn!(
                            error = %e,
                            "a compaction fold failed; its files are orphans under a prefix \
                             CURRENT does not name, nothing retired, and the next trigger \
                             re-plans from scratch"
                        );
                    }
                }
            });
        match spawned {
            Ok(thread) => {
                self.compactions.thread = Some(CompactionThread {
                    abandoned,
                    thread: Some(thread),
                });
                if let Some(last) = self.compactions.last {
                    let ended = self.health.fold_ended_unix.load(Ordering::SeqCst);
                    self.compactions.ended.insert(last, ended);
                }
                self.compactions.last = Some(shard_id);
                if let Some(now) = unix_now() {
                    self.compactions.started.insert(shard_id, now);
                }
            }
            Err(e) => {
                // The closure was dropped without running, which cleared the in-flight flag and
                // ended its record failed before the failure is counted here. No request is
                // answered in between: the executor answers only on this thread, at a later tick.
                self.health.fold_failures.fetch_add(1, Ordering::Relaxed);
                tracing::error!(error = %e, "the OS refused a thread for the compaction fold");
            }
        }
    }

    /// Apply every completed fold waiting from its thread, and report whether any did.
    pub(super) fn publish_completed_folds(&mut self) -> bool {
        // Test hook; always false in a shipped build. See `MaintenanceDeps::switches`.
        if self
            .deps
            .switches
            .fold_publication_paused
            .load(Ordering::SeqCst)
        {
            return false;
        }
        let mut any = false;
        while let Some((written, job)) = self.fold.next_completed() {
            self.publish_fold(written, job);
            any = true;
        }
        if any {
            self.fold.drained();
        }
        // A request is answered as soon as nothing of it is left to run, however its last
        // compaction ended: published, discarded or failed on its thread.
        if !self.fold.outstanding() {
            self.request_answered();
        }
        any
    }

    /// The derived files a compaction's thread wrote that stand for the store as it is now, with
    /// the layout each level's files were written in recorded. A level's files stand where neither
    /// the registry nor the level's versions moved since that thread read them, and `pending` is
    /// the retirement they were composed for or moves the level in neither. Every other file is
    /// removed, and its level recomposes on first use in the layout the registry holds.
    fn settle_derived(
        &mut self,
        prefix_dir: &Path,
        written: Option<DerivedWritten>,
        pending: &PendingRetirement,
    ) -> FoldDerivedFiles {
        self.pending_forms.clear();
        let mut settled = FoldDerivedFiles {
            own: Vec::new(),
            others: Vec::new(),
            moved: Vec::new(),
        };
        let Some(written) = written else {
            return settled;
        };
        let now = self.live.with_artifacts(level_versions);
        let registry = self.live.registry_version() == written.registry;
        let retirement = written.pending.retired == pending.retired;
        let stands = |space: LayerSpace, level: u32| {
            let Some(held) = written.versions.get(&(space, level)) else {
                return false;
            };
            let moves = |pending: &PendingRetirement| pending.is_pending(&held.0, level);
            registry
                && now.get(&(space, level)) == Some(held)
                && (retirement || !(moves(&written.pending) || moves(pending)))
        };
        for (layer, level, chosen) in &written.layouts {
            let Some(space) = self
                .live
                .space_of(layer)
                .filter(|space| stands(*space, *level))
            else {
                continue;
            };
            if self.live.record_layout(layer, *level, *chosen) {
                self.deps.artifact_projections.forget_level(layer, *level);
                settled.moved.push((space, *level));
            }
        }
        let mut dropped: Vec<String> = Vec::new();
        for extent in written.own {
            match stands(extent.space, extent.level) {
                true => settled.own.push(extent),
                false => dropped.push(extent.path),
            }
        }
        for (shard, extent) in written.others {
            match settled.moved.contains(&(extent.space, extent.level)) {
                true => settled.others.push((shard, extent)),
                false => dropped.push(extent.path),
            }
        }
        if !dropped.is_empty() {
            tracing::info!(
                files = dropped.len(),
                "writes landed on levels while the compaction wrote their derived files; those \
                 files are removed, and each such level composes its structures on first use"
            );
            super::background::discard_written(prefix_dir, &dropped);
        }
        settled
    }

    /// Publish a fold: rebase or discard, assemble `SEGMENTS-<n>` from the live partition
    /// manifest, hard-link the carry-forwards, write `MANIFEST.json`, write `SEGMENTS-<n>.json`, flip `CURRENT`, open the
    /// new prefix in-process, swap onto it, rotate the WAL, and reclaim the old prefix.
    ///
    /// What the fold carries forward is read from live state here, hours after the plan, not from
    /// the plan itself. `tombstones` must be a set difference against live state: a delete
    /// accepted during the flight names an entity the fold did not drop, so publishing the plan's
    /// executed set would retire that deletion while its row survives in the rebuilt base.
    ///
    /// `CURRENT` is the commit point. A failure before the flip discards the fold: its files are
    /// orphans, every consumed artefact still stands, and the next trigger re-plans. A failure
    /// after the flip is alarmed instead: the bundle on disc is the new one, but this process goes
    /// on serving the old geometry it still holds mapped, until restarted.
    ///
    /// `job` is the compaction's record, finished once it publishes and failed by a discard.
    pub(super) fn publish_fold(&mut self, written: WrittenFold, job: Option<crate::write::Job>) {
        let WrittenFold { completed, derived } = written;
        let started = std::time::Instant::now();
        let mut stairs = completed.stairs.clone();
        let live = self.generation.load_full();
        let plan = &completed.plan;
        // One closure for every discard below, since each is the same posture: nothing happened,
        // the files are orphans, the next trigger re-plans.
        let discard = {
            let health = Arc::clone(&self.health);
            let prefix = completed.prefix.clone();
            move |reason: &str| {
                health.fold_failures.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    prefix = %prefix,
                    "discarding a completed fold: {reason}. Its files are orphans under a prefix \
                     CURRENT does not name, every consumed artefact still stands, and nothing \
                     retired"
                );
            }
        };
        if let Some(gate) = self.health.unpublishable() {
            discard(gate.detail());
            return;
        }
        // Re-asked here, not just at the plan: a fold's flight is long enough for the WAL to
        // poison after `plan_fold` checked it, and this manifest would then publish deny state no
        // durable record backs. The divergence half is asked by `unpublishable` above.
        if self.log.wal.is_poisoned() {
            discard(
                "the WAL poisoned during its flight, so its manifest would publish deny state no \
                 durable record backs",
            );
            return;
        }

        let FoldApplies {
            forward,
            executed,
            carried_entities,
        } = match fold_applies(&completed, &live) {
            Ok(applies) => applies,
            Err(reason) => {
                discard(&reason);
                return;
            }
        };
        let partition_data = &live.bundle.partitions[&plan.partition];
        let live_manifest = &partition_data.manifest;
        let shard_id = plan.shard;
        let shard = live.shard_of(shard_id);
        let live_record = &shard.data.record;
        let retired_count = executed.cardinality();
        let manifest_n = match self
            .side_manifests
            .allocate_manifest_n(&self.deps.bundle_root, &self.health)
        {
            Ok(n) => n,
            Err(e) => {
                discard(&format!(
                    "its side-manifest number could not be allocated ({e})"
                ));
                return;
            }
        };

        // ---- the artifact pass ---------------------------------------------------------------------
        //
        // Membership extent paths are prefix-relative, so they are written again into the new
        // prefix rather than carried forward or dropped. `repack_all` drops exactly the executed
        // deletions: a suppressed member keeps its bit (Rule S).
        let to_prefix_dir = self.deps.bundle_root.join(&completed.prefix);
        stairs.enter(crate::compact::Pass::Memberships, None);
        let (retired, settled) = retirement(
            &live,
            |space, deleted| self.live.owing(space, deleted),
            shard_id,
            executed.clone(),
        );
        let executed_artifacts = retired.artifacts.clone();
        let repacked =
            match self.rewrite_level_packs(&to_prefix_dir, &plan.partition, manifest_n, &retired) {
                Ok(repacked) => repacked,
                Err(e) => {
                    discard(&format!(
                        "its artifact memberships would not be rewritten ({e})"
                    ));
                    return;
                }
            };

        // The levels the retirement below is about to move, and has not yet
        // ([`PendingRetirement`]): the store still holds pre-retirement records, so a level in the
        // retirement's set got no partition, row column or tile index, and recomposes on first use
        // at the version the retirement will leave it at.
        let pending = PendingRetirement {
            levels: self
                .live
                .with_artifacts(|store| store.levels_moved_by(&retired)),
            retired,
        };
        let FoldDerivedFiles {
            own: derived,
            others: rewritten,
            moved,
        } = self.settle_derived(&to_prefix_dir, derived, &pending);
        stairs.enter(crate::compact::Pass::Report, None);

        // ---- the report, before anything retires ---------------------------------------------------
        //
        // A deletion is not retired before the caller has been told what it degraded: computed
        // here, while the store still holds what this fold is about to retire. A report that
        // cannot be written discards the fold; nothing is lost, since the next fold reports it.
        let degraded = self
            .live
            .with_artifacts(|store| store.degradations(&pending.retired));
        stairs.enter(crate::compact::Pass::Manifest, None);

        // ---- assemble `SEGMENTS-<n>` from the live partition manifest ------------------------------
        //
        // Each view's fold base first, its carried extents after: the reader takes the first
        // segment listed for a view as the one `permutation.bin` addresses.
        let mut segments = Vec::with_capacity(completed.segments.len() + forward.segments.len());
        for base in &completed.segments {
            segments.push(base.clone());
            segments.extend(
                forward
                    .segments
                    .iter()
                    .filter(|d| d.view == base.view)
                    .map(|d| (*d).clone()),
            );
        }
        let freeing = match freeing(shard, &executed) {
            Ok(freeing) => freeing,
            Err(e) => {
                discard(&format!(
                    "the entities it retires could not be told apart from items' numbers ({e})"
                ));
                return;
            }
        };
        let next_tenancy = match write_next_tenancy_index(
            &to_prefix_dir,
            plan.layout(),
            &live_record.tenancy_index,
            &shard.tenancy.raised(&freeing.raised),
        ) {
            Ok(next) => next,
            Err(e) => {
                discard(&format!("its tenancy index would not be written ({e})"));
                return;
            }
        };
        let mut retired_numbers = live_record
            .retired_numbers
            .entities()
            .cloned()
            .unwrap_or_default();
        retired_numbers.or_inplace(&freeing.retired);
        // The suppressions leaving with their entities are logged before the manifest omits
        // them, so a replay of the suppressions the log still holds ends without them. Each entity
        // is deleted until this fold's swap, so the record hides nothing less even if the fold is
        // then discarded.
        let entity = |entity: u32| EntityId::new(u64::from(entity));
        let mut unsuppressed: Vec<(Target, ChangeOp)> = shard
            .overlay
            .suppressions_retired_by(&executed)
            .iter()
            .map(|e| (Target::Item(shard_id, entity(e)), ChangeOp::Unsuppress))
            .collect();
        for (space, deleted) in &executed_artifacts {
            let overlay = live.space_overlay(*space);
            unsuppressed.extend(
                overlay
                    .suppressions_retired_by(deleted)
                    .iter()
                    .map(|e| (Target::Artifact(*space, entity(e)), ChangeOp::Unsuppress)),
            );
        }
        if !unsuppressed.is_empty() {
            let record = WalRecord::ChangeBatch {
                changes: unsuppressed,
            };
            if let Err(failure) = self.append_and_sync(&[&record], None) {
                discard(&format!(
                    "the suppressions leaving with its entities could not be logged ({})",
                    failure.into_error()
                ));
                return;
            }
        }
        // `tombstones` must leave the overlay in the same commit as the manifest, or a manifest
        // carrying what the swap is about to retire would re-seed the overlay at the next restart.
        let mut published_overlay = (*shard.overlay).clone();
        published_overlay.retire(&executed);
        let mut published_spaces = (*live.layer_spaces).clone();
        for (space, deleted) in &executed_artifacts {
            if let Some(parts) = published_spaces.get_mut(space) {
                Arc::make_mut(&mut parts.overlay).retire(deleted);
            }
        }
        // At the log position no later record can name the freed ids at.
        let freed = mosaica_store::manifest::HeldEntities {
            position: self.log.wal.position(),
            entities: mosaica_store::manifest::EntitySet::of(&mosaica_lifecycle::union_of(
                &freeing.ids,
            )),
        };

        let (runtime_attributes, runtime_scoped_attributes) =
            self.live.attributes_for_publication();
        let folded_vocabularies = self
            .live
            .with_vocabularies(|vocabularies| vocabularies.names());
        let (folded_groups, folded_plain_views) = self
            .live
            .with_view_declarations(|declarations| declarations.names());

        // The shard now holds the base of every pair the compaction wrote, so it lists each one
        // a shard lists; a pair of its own it did not write stays while another shard lists it
        // and its view's incarnation is live.
        let mut scoped_columns: Vec<mosaica_store::manifest::ScopedColumn> = Vec::new();
        for (held, column) in partition_data
            .shards
            .iter()
            .flat_map(|other| other.record.scoped_columns.iter().map(move |c| (other, c)))
        {
            let written = completed.scoped_views.contains(column);
            let kept = held.record.id != shard_id
                && live_record.scoped_columns.contains(column)
                && live
                    .bundle
                    .manifest
                    .is_live_incarnation(&column.view, column.incarnation);
            if (written || kept) && !scoped_columns.contains(column) {
                scoped_columns.push(column.clone());
            }
        }
        let runtime_bases: Vec<String> = completed
            .runtime_attributes
            .iter()
            .chain(&completed.runtime_scoped_attributes)
            .cloned()
            .collect();
        let compacted = mosaica_store::manifest::ShardRecord {
            state: live_record.state,
            // Live: from the fold's inputs it would move back past every post-snapshot
            // entity.
            watermark: live_record.watermark,
            high_water: live_record.high_water,
            text_extents: forward.texts.clone(),
            // The artifact pass's own output: paths are prefix-relative, so this is the
            // only list naming files the new prefix contains.
            membership_extents: repacked.members.get(&shard_id).cloned().unwrap_or_default(),
            term_image_extents: completed
                .term_images
                .iter()
                .map(|images| images.extent.clone())
                .collect(),
            segments,
            deltas: forward.tiers.clone(),
            attr_extents: forward.attrs.clone(),
            record_extents: forward.records.clone(),
            entity_terms_extents: forward.entity_terms.clone(),
            unique_indexes: forward
                .unique
                .iter()
                .map(|carried| {
                    let mut index = carried.clone();
                    if let Some((_, base)) = completed
                        .unique
                        .iter()
                        .find(|(attribute, _)| *attribute == carried.attribute)
                    {
                        index.base = base.clone();
                    }
                    index
                })
                .collect(),
            edited_items: mosaica_store::manifest::EditedItemsRuns {
                by_number: mosaica_store::manifest::KeyRuns {
                    base: completed.edited.by_number.base.clone(),
                    live: forward.edited.by_number.live.clone(),
                },
                by_entity: mosaica_store::manifest::KeyRuns {
                    base: completed.edited.by_entity.base.clone(),
                    live: forward.edited.by_entity.live.clone(),
                },
            },
            tenancy_index: next_tenancy.index(),
            retired_numbers: mosaica_store::manifest::EntitySet::of(&retired_numbers),
            scoped_columns,
            runtime_bases,
            ..mosaica_store::manifest::ShardRecord::empty(live_record.id)
        };
        let mut segments_manifest = SegmentsManifest {
            partition: mosaica_store::manifest::PartitionManifest {
                unique_columns: live_manifest.unique_columns.clone(),
                // The live list, not the plan's: a flush that promoted during the flight appended
                // an extent whose ordinals the live dictionary already holds.
                dict_extents: live_manifest.dict_extents.clone(),
                ..mosaica_store::manifest::PartitionManifest::empty()
            },
            shards: mosaica_store::manifest::Shards {
                size: partition_data.shard_size,
                records: Vec::new(),
            },
            layer_spaces: Vec::new(),
        };
        // The compacted shard's record rebuilt; every other shard's as it stands, but for its
        // member packs, which this fold wrote again.
        let mut compacted = Some(compacted);
        for other in partition_data.shards.iter().map(|other| &other.record) {
            let record = match other.id == shard_id {
                true => compacted.take(),
                false => Some(mosaica_store::manifest::ShardRecord {
                    membership_extents: repacked
                        .members
                        .get(&other.id)
                        .cloned()
                        .unwrap_or_default(),
                    ..other.clone()
                }),
            };
            segments_manifest.shards.records.extend(record);
        }
        // The declarations every shard now holds the base of leave for `MANIFEST.json`; the rest,
        // with those made since the plan, stay here.
        let folded = Folded {
            entity: segments_manifest
                .shards
                .based_everywhere(
                    &runtime_attributes
                        .iter()
                        .map(|d| d.name.clone())
                        .collect::<Vec<_>>(),
                )
                .to_vec(),
            scoped: segments_manifest
                .shards
                .based_everywhere(
                    &runtime_scoped_attributes
                        .iter()
                        .map(|f| f.name.clone())
                        .collect::<Vec<_>>(),
                )
                .to_vec(),
        };
        segments_manifest.shards.forget_bases(&folded.entity);
        segments_manifest.shards.forget_bases(&folded.scoped);
        segments_manifest.partition.attributes = runtime_attributes
            .into_iter()
            .filter(|d| !folded.entity.contains(&d.name))
            .collect();
        segments_manifest.partition.scoped_attributes = runtime_scoped_attributes
            .into_iter()
            .filter(|f| !folded.scoped.contains(&f.name))
            .collect();

        write_deny_state(
            &mut segments_manifest,
            shard_overlays(&live).map(|(id, overlay)| match id == shard_id {
                true => (id, &published_overlay),
                false => (id, overlay),
            }),
            &published_spaces,
            |space, deleted| {
                settled
                    .get(&space)
                    .map_or_else(Vec::new, |settled| settled.owed(deleted))
            },
        );

        // ---- the new `MANIFEST.json` ----------------------------------------------------------
        let mut bundle_manifest = self.fold_bundle_manifest(&live, &completed, &folded);
        let declared = bundle_manifest.scoped_scalars();
        segments_manifest.shards.settle_scoped_columns(|pair| {
            declared
                .iter()
                .any(|family| family.name == pair.column && family.views.contains(&pair.view))
        });
        let others = other_shards(
            &plan.partition,
            shard_id,
            live_manifest,
            &live.bundle.manifest.files,
        );
        bundle_manifest.files.extend(others.bundle.clone());
        segments_manifest
            .partition
            .files
            .extend(others.side.clone());

        for (entry, path) in &next_tenancy.written {
            match mosaica_store::digest_of(path) {
                Ok(digest) => {
                    bundle_manifest.files.insert(entry.path.clone(), digest);
                }
                Err(e) => {
                    discard(&format!("its tenancy index would not digest ({e})"));
                    return;
                }
            }
        }
        let mut carried_rels = carried_files(
            plan.layout(),
            live_manifest,
            &live.bundle.manifest.files,
            &forward,
            &next_tenancy.shared,
        );
        for rel in &carried_rels {
            let Some(digest) = live_manifest
                .files
                .get(rel)
                .or_else(|| live.bundle.manifest.files.get(rel))
            else {
                discard("a carried-forward file has no digest in either live manifest");
                return;
            };
            bundle_manifest.files.insert(rel.clone(), digest.clone());
        }

        // ---- link, write, write, flip ---------------------------------------------------------
        let from_prefix_dir = self.prefix_dir(&live);
        // Every other shard's files and the derived files it keeps, linked beside the compacted
        // shard's carry-forwards.
        let mut kept = kept_derived(&self.side_manifests.derived_extents, shard_id, &derived);
        kept.retain(|(_, extent)| !superseded_by_a_move(extent, &moved));
        carried_rels.extend(others.bundle.into_keys());
        carried_rels.extend(others.side.into_keys());
        carried_rels.extend(kept.iter().map(|(_, extent)| extent.path.clone()));
        let mut carried_rels: Vec<String> = carried_rels.into_iter().collect();
        // Content extents carry no entry in either manifest's `files`, so they are linked or
        // written here, from the held lists.
        let contents = match super::publications::carry_space_contents(
            &from_prefix_dir,
            &to_prefix_dir,
            &plan.partition,
            manifest_n,
            &self.side_manifests.spaces,
            &executed_artifacts,
        ) {
            Ok((contents, linked)) => {
                carried_rels.extend(linked);
                contents
            }
            Err(e) => {
                discard(&format!("a content extent would not be rewritten ({e})"));
                return;
            }
        };
        if let Err(e) =
            mosaica_store::hard_link_forward(&from_prefix_dir, &to_prefix_dir, &carried_rels)
        {
            discard(&format!("its carry-forwards would not link ({e})"));
            return;
        }
        // A hard link copies no bytes, and no producer fsyncs a data file on write, so the
        // carry-forwards are synced explicitly here: this fold is about to reclaim the old
        // prefix and rotate the WAL, removing both routes a later flush could recover through.
        let carried_paths: Vec<PathBuf> = carried_rels
            .iter()
            .map(|rel| to_prefix_dir.join(rel))
            .collect();
        if let Err(e) = mosaica_store::fsync_written(&carried_paths) {
            discard(&format!("its carry-forwards would not sync ({e})"));
            return;
        }
        // Written into the new prefix, not linked.
        kept.extend(rewritten);
        // Digested in the new prefix's side-manifest, as a publication's are.
        let artifact_rels = repacked
            .members
            .values()
            .flatten()
            .map(|extent| extent.path.clone())
            .chain(
                repacked
                    .records
                    .values()
                    .flatten()
                    .map(|extent| extent.path.clone()),
            )
            .chain(
                contents
                    .values()
                    .flatten()
                    .flat_map(|extent| extent.files().map(String::from)),
            );
        let mut artifact_files = std::collections::BTreeMap::new();
        for rel in artifact_rels {
            match mosaica_store::digest_of(&to_prefix_dir.join(&rel)) {
                Ok(digest) => {
                    artifact_files.insert(rel, digest);
                }
                Err(e) => {
                    discard(&format!("an artifact pack would not digest ({e})"));
                    return;
                }
            }
        }
        segments_manifest
            .partition
            .files
            .extend(artifact_files.clone());
        let manifest_digest =
            match mosaica_store::write_manifest_json(&to_prefix_dir, &bundle_manifest) {
                Ok(digest) => digest,
                Err(e) => {
                    discard(&format!("its MANIFEST.json would not commit ({e})"));
                    return;
                }
            };
        // The compacted shard's identity moves with the prefix's `MANIFEST.json`.
        if segments_manifest
            .stamp_identity(plan.shard, &manifest_digest)
            .is_none()
        {
            discard("its MANIFEST.json digest is not 64 hexadecimal digits");
            return;
        }
        let fold_refs = match self.commit_side_manifest(
            partition_data,
            &to_prefix_dir,
            &plan.partition,
            manifest_n,
            &mut segments_manifest,
            Some(FoldDerived {
                shard: plan.shard,
                written: &derived,
                kept,
                records: &repacked.records,
                contents: &contents,
                pending_retirement: &pending,
                freed: &freed,
            }),
        ) {
            Ok(refs) => refs,
            Err(e) => {
                discard(&format!(
                    "its SEGMENTS-{manifest_n}.json would not commit ({e})"
                ));
                return;
            }
        };
        // The commit point. Everything above is reversible; nothing below is. A kill parked before
        // this line leaves a complete, synced `v#####` tree `CURRENT` never named, and the startup
        // sweep reclaims it whole.
        //
        // The report must be written before this line: it says a deletion's obligation was
        // discharged, so writing it after would let a fold abandoned at the flip leave a false one.
        if let Err(e) = self.write_fold_report(&completed.prefix, &degraded) {
            discard(&format!(
                "its degradation report would not be written ({e}), and a deletion may not retire \
                 before the caller has been told what it degraded"
            ));
            return;
        }
        stairs.enter(crate::compact::Pass::Flip, None);

        self.pause_point(PauseSiteArg::BeforeCurrentFlip);
        if let Err(e) = mosaica_store::write_current(
            &self.deps.bundle_root,
            &completed.prefix,
            &manifest_digest,
        ) {
            discard(&format!("CURRENT would not flip ({e})"));
            return;
        }
        stairs.enter(crate::compact::Pass::Retire, None);

        // Retires the resident store here, before the new generation is installed: the published
        // overlay already carries the executed deletions retired, so retiring after would leave a
        // window where a deleted artifact's entity reads not denied while its record still serves.
        let mut moved = self.live.retire_artifacts(&pending.retired);
        self.live.settle_owing(settled);
        self.live.mark_memberships_published();
        self.live.mark_growth_packed();
        let (rehoused, kept) = self
            .live
            .rehouse_memberships(&to_prefix_dir, &repacked.members);
        if kept > 0 {
            tracing::error!(
                rehoused,
                kept,
                "ALARM: artifact memberships this fold wrote back do not match the records they \
                 were written from; the level the prefix carries and the level being served \
                 disagree for those ordinals"
            );
        }
        self.side_manifests.membership_extents = repacked.members;
        self.side_manifests.artifact_files = artifact_files;
        self.side_manifests
            .refs
            .insert(plan.partition.clone(), fold_refs);
        self.side_manifests.spaces = segments_manifest
            .layer_spaces
            .iter()
            .map(|record| (record.space, HeldSpace::of(record)))
            .collect();
        // Checked, not assumed: a pending level's structures were stamped with the version the
        // level would have after this retirement ([`PendingRetirement`]). If `retire` ever moves a
        // different set, a stamped structure would be carried at a version it does not describe.
        moved.sort();
        let mut expected = pending.levels.clone();
        expected.sort();
        if moved != expected {
            tracing::error!(
                stamped_for = ?expected,
                moved = ?moved,
                "ALARM: the fold's retirement moved levels other than the ones its derived \
                 structures were stamped for; every structure whose stamp the store does not \
                 carry is dropped and its level recomposes on first use"
            );
        }
        let written: Vec<(ShardId, mosaica_store::manifest::DerivedExtent)> = segments_manifest
            .derived_by_shard()
            .map(|(shard, extent)| (shard, extent.clone()))
            .collect();
        let enumerated = mosaica_types::layer::enumerated_names(&self.live.registered_layers());
        self.side_manifests.derived_extents = self
            .live
            .with_artifacts(|store| standing_after_retirement(store, &enumerated, written));
        let folded_tiers = segments_manifest
            .shard(shard_id)
            .map(|record| record.deltas.clone())
            .unwrap_or_default();
        *lock_recover(&self.health.last_fold_report) = degraded;
        stairs.enter(crate::compact::Pass::Open, None);

        let segments_version = live.segments_version + 1;
        if let Err(reason) = self.swap_onto_folded_prefix(
            &completed.prefix,
            &live,
            shard_id,
            &folded_tiers,
            executed,
            executed_artifacts,
            &completed.tallies,
        ) {
            self.diverge_from_current(&completed.prefix, &reason);
            return;
        }
        // Taken here, while every generation in it is over the prefix this fold supersedes: one
        // published after the swap is over the new prefix, and the next fold waits for it.
        let mut generations =
            std::mem::take(&mut *self.superseded.lock().unwrap_or_else(|e| e.into_inner()));
        generations.push(Arc::downgrade(&live));
        stairs.enter(crate::compact::Pass::Adopt, None);

        // Only the files written over a view's live incarnation, as an open adopts them.
        let swapped = self.generation.load();
        let incarnation_of = |view: &str| swapped.bundle.manifest.incarnation_of(view);
        let derived: Vec<_> = self
            .side_manifests
            .derived_extents
            .iter()
            .flat_map(|(shard, extents)| extents.iter().map(|extent| (*shard, extent.clone())))
            .filter(|(_, extent)| {
                crate::filter::carries_live_view(
                    &incarnation_of,
                    extent.view.as_deref(),
                    extent.incarnation,
                )
            })
            .collect();
        self.live.with_artifacts(|store| {
            self.deps.artifact_projections.adopt_derived(
                &to_prefix_dir,
                &completed.prefix,
                &derived,
                store,
                &enumerated,
            )
        });
        stairs.enter(crate::compact::Pass::Warm, None);

        // Must run after both the swap and the retire above: built before the swap, a projection
        // would be keyed to a generation no reader can ask for; built before the retire, it would
        // be keyed to a store version the retire is about to bump, discarding the warm.
        self.warm_artifact_caches();
        self.deps.shapes.clear_staged();
        stairs.enter(crate::compact::Pass::Wal, None);

        // ---- rotate the WAL ---------------------------------------------------------------------
        //
        // Immediately: the manifest seed no longer carries the executed entries, but the WAL still
        // holds the original delete records until a rotation reclaims them. The freed ids are
        // issued once it has.
        let freed_ids = freed
            .entities
            .entities()
            .map_or(0, croaring::Bitmap::cardinality);
        let freed_numbers = freeing.raised.cardinality();
        let exhausted_numbers = freeing.retired.cardinality();
        if let Some(compacted) = self.live.shard(shard_id) {
            compacted.with_allocator(|alloc| alloc.release_after(freed.position, freeing.ids));
        }
        self.rotate_wal();
        stairs.enter(crate::compact::Pass::Reclaim, None);
        self.live.with_attributes(|attributes| {
            attributes.retire_folded(&folded.entity, &folded.scoped)
        });
        self.live
            .with_vocabularies(|vocabularies| vocabularies.retire_folded(&folded_vocabularies));
        self.live.with_view_declarations(|declarations| {
            declarations.retire_folded(&folded_groups, &folded_plain_views)
        });

        // ---- reclaim the superseded prefix -----------------------------------------------------
        let CarriedExtents {
            omitted_views,
            omitted_segments,
            ..
        } = forward;
        drop(live);
        self.pending_reclaim.push(PendingReclaim {
            generations,
            prefix_dir: from_prefix_dir,
        });
        self.reclaim_superseded_prefixes();
        let cost = stairs.finish();
        if let Some(job) = job {
            job.finish();
        }

        let (passes, fold_secs, staircase_rss) = self.health.record_fold_cost(
            shard_id,
            cost,
            completed.attr_bytes_read,
            completed.attr_bytes_written,
        );
        // After the cost, so a reader that sees the count move reads this fold's passes.
        self.health.folds.fetch_add(1, Ordering::Relaxed);
        // One line per view: a group's keys are separate views over one dictionary, each paying
        // its own table and payload.
        for images in &completed.term_images {
            let summary = &images.summary;
            tracing::info!(
                prefix = %completed.prefix,
                view = %images.extent.view,
                kept = summary.kept,
                terms = summary.terms,
                payload_bytes = summary.payload_bytes,
                table_bytes = summary.table_bytes,
                wall_s = summary.wall.as_secs_f64(),
                "a fold wrote a view's term images"
            );
        }
        tracing::info!(
            prefix = %completed.prefix,
            shard = %shard_id,
            segments_version,
            retired = retired_count,
            freed_ids,
            freed_numbers,
            exhausted_numbers,
            carried_entities,
            elapsed_ms = started.elapsed().as_millis() as u64,
            passes = %passes,
            fold_secs,
            staircase_rss,
            attr_bytes_read = completed.attr_bytes_read,
            attr_bytes_written = completed.attr_bytes_written,
            dropped_views = %if omitted_views.is_empty() {
                "none".to_string()
            } else {
                omitted_views.join(",")
            },
            dropped_view_segments = omitted_segments,
            "a compaction published: the shard is one base segment per view, one base postings \
             tier, plus whatever landed during its flight"
        );
    }

    /// The `MANIFEST.json` a fold's new prefix carries: the live one, less every column declared
    /// at a running service but those `folded`, with every live vocabulary binding folded in, and
    /// the fold's own files.
    ///
    /// The vocabulary bindings fold in verbatim, never re-derived, re-sorted or re-numbered: keys
    /// and codes are byte-identical everywhere, and `columns.arrow` stores the code alone, so
    /// renumbering here would recolour the corpus with no error and no digest mismatch.
    fn fold_bundle_manifest(
        &self,
        live: &Generation,
        completed: &crate::compact::CompletedFold,
        folded: &Folded,
    ) -> mosaica_store::manifest::Manifest {
        let mut bundle_manifest = live.bundle.manifest.clone();
        let (runtime_attributes, runtime_scoped_attributes) =
            self.live.attributes_for_publication();
        bundle_manifest.declared_scalars.retain(|d| {
            !runtime_attributes
                .iter()
                .any(|runtime| runtime.name == d.name && !folded.entity.contains(&d.name))
        });
        for group in &mut bundle_manifest.groups {
            group.scoped_scalars.retain(|f| {
                !runtime_scoped_attributes
                    .iter()
                    .any(|runtime| runtime.name == f.name && !folded.scoped.contains(&f.name))
            });
        }
        crate::vocabularies::merge_live_values(&mut bundle_manifest, &live.vocabularies);
        bundle_manifest.files = completed.files.clone();
        let carried_bindings: Vec<_> = live
            .bundle
            .partitions
            .values()
            .flat_map(|partition| partition.manifest.vocabulary_extensions.iter().cloned())
            .collect();
        mosaica_store::vocabulary::fold_extensions_into(
            &mut bundle_manifest.vocabularies,
            &carried_bindings,
        );
        bundle_manifest
    }

    /// Latch [`ExecutorHealth::prefix_diverged`]: `CURRENT` names a prefix this process could not
    /// swap onto, so it must stop writing durable state.
    ///
    /// A publication after this point would write under the old prefix (which the live generation
    /// still names) and then rotate the WAL, acking ingest that a restart loses. The superseded
    /// prefix is not reclaimed here: this process still serves from it.
    pub(super) fn diverge_from_current(&self, committed: &str, reason: &str) {
        self.health.fold_failures.fetch_add(1, Ordering::Relaxed);
        self.health.prefix_diverged.store(true, Ordering::SeqCst);
        tracing::error!(
            committed_prefix = %committed,
            "ALARM: CURRENT names the folded prefix and this process could not swap onto it: \
             {reason}. It keeps serving the superseded prefix and publishes nothing (no geometry, \
             no deny state, no WAL rotation) until it is restarted, when it opens the committed \
             prefix. Restart this node"
        );
    }

    /// Opens the prefix `CURRENT` now names and swaps onto it, retiring `retired` from the shard's
    /// overlay and `retired_artifacts` from each space's. The shard's views take `tallies`, the
    /// field tallies the compaction wrote.
    #[allow(clippy::too_many_arguments)]
    fn swap_onto_folded_prefix(
        &mut self,
        prefix: &str,
        live: &Arc<Generation>,
        shard_id: ShardId,
        folded_tiers: &[String],
        retired: croaring::Bitmap,
        retired_artifacts: std::collections::BTreeMap<LayerSpace, croaring::Bitmap>,
        tallies: &mosaica_store::field_tallies::ViewTallies,
    ) -> Result<(), String> {
        let shard = live.shard_of(shard_id);
        let live_tiers = &shard.data.record.deltas;
        let (bundle, mut rotation) = crate::engine::open_rotation(
            &self.deps.bundle_root,
            prefix,
            shard_id,
            retired,
            Some((&live.bundle, tallies)),
        )
        .map_err(|e| format!("the folded prefix would not open ({e})"))?;
        rotation.retired_artifacts = retired_artifacts;
        let delta_postings = folded_tiers
            .iter()
            .map(|rel| {
                held_tier(&shard.delta_postings, live_tiers, rel).ok_or_else(|| {
                    format!("the folded manifest names delta tier {rel}, which is not held open")
                })
            })
            .collect::<Result<Vec<Arc<DeltaTier>>, String>>()?;
        self.publish_geometry(
            GeometryPublication::within_prefix(
                shard_id,
                prefix.to_string(),
                live.segments_version + 1,
                shard.watermark,
                bundle,
                Arc::clone(&live.dict),
                delta_postings,
            )
            .rotating(rotation),
        )
        .map_err(|e| format!("the swap was refused ({e})"))
    }

    /// Delete every superseded prefix nothing is reading any more.
    ///
    /// Waits for every generation published over the prefix to be released before unlinking: the
    /// generation pointer has already moved, so no new holder can appear.
    ///
    /// A failure alarms once and drops the entry rather than retrying every tick; the tree stands
    /// as an orphan.
    pub(super) fn reclaim_superseded_prefixes(&mut self) {
        if self.pending_reclaim.is_empty() {
            return;
        }
        let mut still_read = Vec::new();
        for pending in std::mem::take(&mut self.pending_reclaim) {
            if pending
                .generations
                .iter()
                .any(|held| held.strong_count() > 0)
            {
                still_read.push(pending);
                continue;
            }
            let prefix_dir = pending.prefix_dir;
            match mosaica_store::reclaim_prefix(&prefix_dir) {
                Ok(()) => tracing::info!(
                    prefix = %prefix_dir.display(),
                    "the superseded prefix is reclaimed: the build's base, every merged-away \
                     segment, every consumed tier and every superseded side-manifest"
                ),
                Err(e) => tracing::error!(
                    error = %e,
                    prefix = %prefix_dir.display(),
                    "ALARM: the superseded prefix could not be reclaimed and stands as an orphan. \
                     Nothing live was unlinked; what is lost is the disc the fold exists to free"
                ),
            }
        }
        self.pending_reclaim = still_read;
    }

    /// Write the fold's degradation report, and keep the last one for the operator route.
    ///
    /// Written to `reports/` in the bundle root, not into the prefix, which a later fold reclaims.
    /// A report with nothing in it is still written, so an operator can tell "degraded nothing"
    /// from "never reported".
    pub(super) fn write_fold_report(
        &self,
        prefix: &str,
        degraded: &[mosaica_lifecycle::membership::Degradation],
    ) -> std::io::Result<()> {
        let dir = self.deps.bundle_root.join("reports");
        std::fs::create_dir_all(&dir)?;
        let rows: Vec<serde_json::Value> = degraded
            .iter()
            .map(|d| {
                serde_json::json!({
                    "layer": d.layer,
                    "level": d.level,
                    "ordinal": d.ordinal,
                    "key": d.key,
                    "members_lost": d.members_lost,
                    "declared_members": d.declared_members,
                    "contents_lost": d
                        .contents_lost
                        .iter()
                        .map(|(index, lost)| serde_json::json!({"rank": index, "lost": lost}))
                        .collect::<Vec<_>>(),
                })
            })
            .collect();
        let body = serde_json::json!({
            "prefix": prefix,
            "degraded": rows,
        });
        let bytes = serde_json::to_vec_pretty(&body)?;
        let path = dir.join(format!("fold-{prefix}.json"));
        mosaica_store::write_and_fsync(&path, &bytes)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        mosaica_store::fsync_dir(&dir).map_err(|e| std::io::Error::other(e.to_string()))?;
        // The in-memory copy is set by the caller, after the flip, not here: the fold can still
        // be discarded after this file is written.
        Ok(())
    }

    /// The fold's artifact pass: write every level whole into the prefix being published, with the
    /// fold's executed deletions dropped from each membership.
    ///
    /// The returned list replaces the manifest's rather than extending it: one extent per level,
    /// covering `[0, len)`. Holes are written, not packed around: closing one would hand every
    /// later artifact in the level the identity of its neighbour, since an ordinal is the identity
    /// a caller's `mosaica_id` resolves to.
    pub(super) fn rewrite_level_packs(
        &self,
        prefix_dir: &std::path::Path,
        partition: &str,
        n: u64,
        retired: &mosaica_lifecycle::membership::Retired,
    ) -> mosaica_store::Result<PackedLevels> {
        let ready = self.live.with_artifacts(|store| store.repack_all(retired));
        pack_levels(prefix_dir, partition, n, ready, |layer| {
            self.live.space_of(layer)
        })
    }
}

impl DerivedSources {
    /// The derived files of the prefix `completed` wrote, written on the compaction's thread
    /// before its hand-off, for the publication to settle against the store as it then stands
    /// ([`Executor::settle_derived`]). `None` where the fold no longer applies to the live bundle,
    /// which its publication then discards.
    ///
    /// The store is only read. Each level's versions are taken before and after the pass, and a
    /// level written meanwhile is not carried, since its files may describe either side.
    fn write(
        &self,
        completed: &crate::compact::CompletedFold,
        prefix_dir: &std::path::Path,
    ) -> Option<DerivedWritten> {
        let live = self.generation.load_full();
        let applies = fold_applies(completed, &live).ok()?;
        let registry = self.live.registry_version();
        let before = self.live.with_artifacts(level_versions);
        let (retired, _) = retirement(
            &live,
            |space, deleted| self.live.owing_unnoted(space, deleted),
            completed.plan.shard,
            applies.executed,
        );
        let pending = PendingRetirement {
            levels: self
                .live
                .with_artifacts(|store| store.levels_moved_by(&retired)),
            retired,
        };
        let (own, others, layouts) =
            self.write_fold_derived(&live, prefix_dir, completed, &pending);
        let after = self.live.with_artifacts(level_versions);
        let mut versions = before;
        versions.retain(|level, held| after.get(level) == Some(held));
        Some(DerivedWritten {
            own,
            others,
            layouts,
            pending,
            versions,
            registry,
        })
    }

    /// Writes every derived artifact file into the prefix the fold is publishing, and returns them
    /// with the layout chosen for each level. All of them are derived, so a file that will not
    /// compose is left out and built on first use. A level whose chosen layout is not the one the
    /// registry holds has every other shard's column written again in the new one.
    fn write_fold_derived(
        &self,
        live: &Generation,
        to_prefix_dir: &std::path::Path,
        completed: &crate::compact::CompletedFold,
        pending: &PendingRetirement,
    ) -> (
        Vec<mosaica_store::manifest::DerivedExtent>,
        Vec<(ShardId, mosaica_store::manifest::DerivedExtent)>,
        Vec<ChosenLayout>,
    ) {
        // Every file derives from a level, so with none the prefix's postings, permutations and
        // segments are not read.
        if self
            .live
            .with_artifacts(|store| store.levels_and_extents().next().is_none())
        {
            return (Vec::new(), Vec::new(), Vec::new());
        }
        let plan = &completed.plan;
        let views: Vec<(String, u32)> = completed
            .segments
            .iter()
            .map(|segment| (segment.view.clone(), segment.row_count))
            .collect();
        let layout = plan.layout();
        let mut pass = DerivedPass {
            prefix_dir: to_prefix_dir,
            layout,
            n: self.n,
            incarnations: plan
                .views
                .iter()
                .map(|view| (view.view.clone(), view.incarnation))
                .collect(),
            spaces: self.fold_row_spaces(to_prefix_dir, layout, &views),
            segments: fold_segments(to_prefix_dir, layout, &completed.segments),
            pending,
            index: Default::default(),
        };
        let mut derived = self.write_containment_partitions(&mut pass);
        let (layouts, overlapping) =
            self.choose_layouts(live, layout.shard, &pass.spaces, pending, &pass.segments);
        let moved: Vec<ChosenLayout> = layouts
            .iter()
            .filter(|(layer, level, chosen)| {
                self.live
                    .registered_layer(layer)
                    .is_some_and(|registered| registered.layout_of(*level) != *chosen)
            })
            .cloned()
            .collect();
        derived.extend(self.write_tile_indexes(&mut pass, &layouts));
        derived.extend(self.write_row_columns(&mut pass, &layouts, &overlapping));
        let shape_rows = self.write_shape_rows(&mut pass, &derived);
        derived.extend(shape_rows);
        derived.extend(self.write_shape_held(&mut pass));
        let others = self.write_moved_columns(live, &pass, &moved, &overlapping);
        (derived, others, layouts)
    }

    /// Every shard but the compacted one's columns of each `moved` level, written into the
    /// compaction's prefix in the level's new layout from that shard's live rows.
    fn write_moved_columns(
        &self,
        live: &Generation,
        compacted: &DerivedPass<'_>,
        moved: &[(String, u32, mosaica_types::layer::ServingLayout)],
        overlapping: &Overlapping,
    ) -> Vec<(ShardId, mosaica_store::manifest::DerivedExtent)> {
        let mut written = Vec::new();
        if moved.is_empty() {
            return written;
        }
        for other in live
            .shards()
            .filter(|other| other.id() != compacted.layout.shard)
        {
            let mut spaces = Vec::new();
            let mut segments = Vec::new();
            for (view, _) in &compacted.spaces {
                let Some(data) = other.data.views.get(view) else {
                    continue;
                };
                spaces.push((view.clone(), data.row_space.clone()));
                if data.row_space.base_rows() > 0 {
                    if let Some(base) = data.segments.first() {
                        segments.push((view.clone(), Arc::clone(base)));
                    }
                }
                for (layer, level, _) in moved {
                    if self.live.spatial_layer(layer) {
                        self.stage_level(other.id(), view, data, layer, *level);
                    }
                }
            }
            let mut pass = DerivedPass {
                prefix_dir: compacted.prefix_dir,
                layout: ShardLayout::new(compacted.layout.partition, other.id()),
                n: compacted.n,
                incarnations: compacted.incarnations.clone(),
                spaces,
                segments,
                pending: compacted.pending,
                index: Default::default(),
            };
            let columns = self.write_row_columns(&mut pass, moved, overlapping);
            written.extend(columns.into_iter().map(|extent| (other.id(), extent)));
        }
        written
    }

    /// Compose and write this prefix's containment partitions, one file per `(layer, level)`.
    ///
    /// Composed against the prefix being published, not the one being left, over the postings that
    /// prefix's own artifact pass just wrote. Every failure is an empty list, not a discarded
    /// fold: a partition is derived, so the level recomposes it on first use.
    fn write_containment_partitions(
        &self,
        pass: &mut DerivedPass<'_>,
    ) -> Vec<mosaica_store::manifest::DerivedExtent> {
        let prefix_dir = pass.prefix_dir;
        let layout = pass.layout;
        let n = pass.n;
        let pending = pass.pending;
        let postings_path = prefix_dir.join(layout.postings_rel());
        let postings = match mosaica_authz::PostingsReader::open(&postings_path, true) {
            Ok(postings) => postings,
            Err(error) => {
                tracing::warn!(
                    path = %postings_path.display(),
                    %error,
                    "the fold could not read the prefix it just wrote to compose containment \
                     partitions; every level recomposes on first use"
                );
                return Vec::new();
            }
        };

        // The store's lock is taken one level at a time, so a reader waits for one level at most.
        let levels: Vec<(String, u32)> =
            self.live.with_artifacts(|store| levels_of(store).collect());
        let composed: Vec<(String, u32, u64, Vec<u8>)> = levels
            .into_iter()
            .filter(|(layer, level)| !pending.is_pending(layer, *level))
            .filter_map(|(layer, level)| {
                let composed = self.live.with_artifacts(|store| {
                    let slice = slice_of(store, layout.shard);
                    let version = slice.level_version(&layer, level);
                    crate::containment::ContainmentPartition::compose_slice(
                        &crate::artifacts::LevelSlice::new(slice, store, &layer, level, None),
                        &postings,
                    )
                    .map(|partition| (version, partition.as_bytes().to_vec()))
                });
                match composed {
                    Ok((version, bytes)) => Some((layer, level, version, bytes)),
                    Err(error) => {
                        tracing::warn!(
                            layer = %layer,
                            level,
                            %error,
                            "a containment partition would not compose at the fold; that level \
                             recomposes on first use"
                        );
                        None
                    }
                }
            })
            .collect();
        if composed.is_empty() {
            return Vec::new();
        }

        mosaica_store::derived::file_derived(
            prefix_dir,
            layout,
            n,
            &mut pass.index,
            composed
                .into_iter()
                .filter_map(|(layer, level, level_version, bytes)| {
                    Some(mosaica_store::derived::Filed {
                        view: None,
                        incarnation: None,
                        space: self.live.space_of(&layer)?,
                        level,
                        level_version: self.live.with_artifacts(|store| {
                            pending.records_version_after(store, &layer, level)
                        }),
                        slice_version: level_version,
                        form: mosaica_store::manifest::DerivedForm::Containment,
                        bytes: mosaica_store::derived::FiledBytes::InHand(bytes),
                    })
                })
                .collect(),
        )
    }

    /// The base row space of every view this fold just wrote, opened once for the whole artifact
    /// pass: base only, with no extents, which is what every structure derived from one is
    /// computed over.
    ///
    /// A view whose permutation will not load is simply absent: its structures are derived on
    /// first use, the same as before the fold wrote anything.
    pub(super) fn fold_row_spaces(
        &self,
        prefix_dir: &std::path::Path,
        layout: ShardLayout<'_>,
        views: &[(String, u32)],
    ) -> Vec<(String, mosaica_store::RowSpace)> {
        let mut spaces: Vec<(String, mosaica_store::RowSpace)> = Vec::new();
        for (view, row_count) in views {
            let path = prefix_dir.join(layout.permutation_rel(view));
            match mosaica_store::Permutation::load(&path) {
                Ok(permutation) => spaces.push((
                    view.clone(),
                    mosaica_store::RowSpace::new(std::sync::Arc::new(permutation), *row_count),
                )),
                Err(error) => tracing::warn!(
                    view = %view,
                    path = %path.display(),
                    %error,
                    "the fold could not read the permutation it just wrote, so this view's derived \
                     artifact structures are built on first use"
                ),
            }
        }
        spaces
    }

    /// Whether the artifact pass composes a level's row column and tile index: every level the
    /// retirement does not move, and an enumerated level it does. A spatial level the retirement
    /// moves is omitted, since its rows resolve from shapes instead.
    pub(super) fn composes_row_structures(
        &self,
        pending: &PendingRetirement,
        layer: &str,
        level: u32,
    ) -> bool {
        !pending.is_pending(layer, level)
            || self.live.registered_layer(layer).is_some_and(|registered| {
                registered.declaration.membership
                    == mosaica_types::layer::MembershipSource::Enumerated
            })
    }

    /// `layer`'s spatial `level` in `view`, with a piece staged for every segment of `shard`'s
    /// `data` it held none of.
    fn stage_level(
        &self,
        shard: ShardId,
        view: &str,
        data: &mosaica_store::read::ViewData,
        layer: &str,
        level: u32,
    ) -> Arc<crate::shapes::ShapeLevel> {
        self.live.with_artifacts(|store| {
            let held = self.shapes.level(
                view,
                data.incarnation,
                layer,
                level,
                store,
                &crate::shapes::PersistedPieces::none(),
            );
            crate::shapes::ShapeStore::stage_missing(
                &held,
                shard,
                data,
                &crate::shapes::PersistedPieces::none(),
                store,
            );
            held
        })
    }

    /// `layer`'s `level` in shard `other` as it stands, observed over `view`'s rows: its shape and
    /// the ordinals holding a row there. A spatial level's rows are the form the shard serves where
    /// it holds them, and its segments resolved against the shapes otherwise.
    fn observed_in(
        &self,
        other: &crate::Shard,
        view: &str,
        layer: &str,
        level: u32,
        spatial: bool,
        pending: &PendingRetirement,
    ) -> Option<(crate::layout::LevelShape, croaring::Bitmap)> {
        let data = other.data.views.get(view)?;
        let space = &data.row_space;
        let row_count = u32::try_from(space.total_rows()).unwrap_or(u32::MAX);
        if !spatial {
            let members = self
                .live
                .with_artifacts(|store| held_members(store, pending, other.id(), layer, level));
            return Some(observe_held(row_count, &|visit| {
                for (ordinal, members) in &members {
                    visit(*ordinal, &members.projected(|part| space.project(part)));
                }
            }));
        }
        let form = self
            .projections
            .held_form(other.id(), view, layer, level)
            .filter(|form| form.membership().rows_held());
        if let Some(form) = form {
            return Some(observe_held(row_count, &|visit| {
                for ordinal in 0..form.len() as u32 {
                    if let Some(rows) = form.get(ordinal) {
                        visit(ordinal, rows);
                    }
                }
            }));
        }
        let segments = crate::viewport::segments_with_row_bases(view, data).ok()?;
        let held = self.stage_level(other.id(), view, data, layer, level);
        let mut rows: Vec<Option<croaring::Bitmap>> = Vec::new();
        for (segment, base) in &segments {
            let Some(piece) = held.staged(other.id(), &segment.seg_id) else {
                continue;
            };
            if rows.len() < piece.len() {
                rows.resize(piece.len(), None);
            }
            for (slot, part) in rows.iter_mut().zip(piece.iter()) {
                if let Some(part) = part.as_ref().filter(|part| !part.is_empty()) {
                    slot.get_or_insert_with(croaring::Bitmap::new)
                        .or_inplace(&part.add_offset(i64::from(*base)));
                }
            }
        }
        Some(observe_held(row_count, &|visit| {
            for (ordinal, rows) in rows.iter().enumerate() {
                if let Some(rows) = rows {
                    visit(ordinal as u32, rows);
                }
            }
        }))
    }

    /// The fold's layout re-evaluation, run inside the artifact pass, reading the store
    /// post-retirement so a level the fold retired most of is observed as it is about to become.
    ///
    /// The record is per `(layer, level)` but the observation is per view; the shape is taken in
    /// the first view the fold wrote, deterministically, and the other views' columns are written
    /// in that layout even where it is not the one they would have chosen alone. Beside the layouts,
    /// the levels whose observed memberships overlap.
    pub(super) fn choose_layouts(
        &self,
        live: &Generation,
        shard: ShardId,
        spaces: &[(String, mosaica_store::RowSpace)],
        pending: &PendingRetirement,
        fold_segments: &[(String, Arc<mosaica_store::read::SegmentData>)],
    ) -> (
        Vec<(String, u32, mosaica_types::layer::ServingLayout)>,
        Overlapping,
    ) {
        let mut overlapping = std::collections::BTreeSet::new();
        let Some((first_view, space)) = spaces.first() else {
            return (Vec::new(), overlapping);
        };
        // The registry is read outside the store's lock throughout, since a publication takes
        // the two in the other order.
        let mut levels: Vec<(String, u32)> =
            self.live.with_artifacts(|store| levels_of(store).collect());
        levels.retain(|(layer, level)| self.composes_row_structures(pending, layer, *level));
        let mut out = Vec::with_capacity(levels.len());
        let mut per_layer: std::collections::BTreeMap<String, Vec<(u32, u64)>> =
            std::collections::BTreeMap::new();
        for (layer, level) in levels {
            let Some(registered) = self.live.registered_layer(&layer) else {
                continue;
            };
            // A spatial level is observed over its resolved rows: every segment the fold wrote is
            // resolved against the level's shapes here, and the pieces are staged under the new
            // segment ids for the derived files this pass writes and for the flip's warm.
            let spatial = spatial_membership(&registered.declaration);
            // The store's lock is held to read the level alone, and released before the level is
            // resolved or projected, so a write waits for the read and no more.
            let (shape, held) = if spatial {
                let mut observed = None;
                for (view, segment) in fold_segments {
                    let Some(incarnation) = live.bundle.manifest.incarnation_of(view) else {
                        continue;
                    };
                    let held = self.live.with_artifacts(|store| {
                        self.shapes.level(
                            view,
                            incarnation,
                            &layer,
                            level,
                            store,
                            &crate::shapes::PersistedPieces::none(),
                        )
                    });
                    let (piece, cost) = held.resolve(segment);
                    let piece = Arc::new(piece);
                    held.stage(shard, &segment.seg_id, Arc::clone(&piece));
                    tracing::info!(
                        layer = %layer,
                        level,
                        view = %view,
                        seg_id = %segment.seg_id,
                        rows_tested = cost.rows_tested,
                        rows_interior = cost.rows_interior,
                        artifacts_empty = cost.artifacts_empty,
                        elapsed_ms = cost.elapsed_ms,
                        "the fold re-resolved a segment against a spatial level's shapes"
                    );
                    if view == first_view {
                        observed = Some(observe_held(space.base_rows(), &|visit| {
                            for (ordinal, rows) in piece.iter().enumerate() {
                                if let Some(rows) = rows {
                                    visit(ordinal as u32, rows);
                                }
                            }
                        }));
                    }
                }
                observed.unwrap_or_else(|| {
                    (crate::layout::LevelShape::empty(), croaring::Bitmap::new())
                })
            } else {
                // A borrowing level's filed column is true when written and not read
                // afterwards: the borrowed set moves with the target's version, so the engine
                // serves it artifact-major instead (`crate::artifacts::ArtifactRows::inherit`).
                let members = self
                    .live
                    .with_artifacts(|store| held_members(store, pending, shard, &layer, level));
                observe_held(space.base_rows(), &|visit| {
                    for (ordinal, members) in &members {
                        visit(
                            *ordinal,
                            &members.projected(|part| space.project_base(part)),
                        );
                    }
                })
            };
            // The level over every shard: each other shard as it stands, beside this one as the
            // compaction leaves it.
            let mut reached = held;
            let mut shapes = vec![shape];
            for other in live.shards().filter(|other| other.id() != shard) {
                let observed = self.observed_in(other, first_view, &layer, level, spatial, pending);
                if let Some((shape, held)) = observed {
                    reached.or_inplace(&held);
                    shapes.push(shape);
                }
            }
            let shape = crate::layout::LevelShape::summed(reached.cardinality(), shapes);
            let chosen = crate::layout::choose(&registered.declaration, shape);
            tracing::info!(
                layer = %layer,
                level,
                artifacts = shape.artifacts,
                everywhere_fraction = shape.everywhere_fraction,
                blocks_per_artifact = shape.blocks_per_artifact,
                partitions = shape.partitions,
                pinned = ?registered.declaration.layout,
                was = ?registered.layout_of(level),
                now = ?chosen,
                "the fold re-evaluated a level's serving layout"
            );
            per_layer
                .entry(layer.clone())
                .or_default()
                .push((level, shape.artifacts));
            if !shape.partitions {
                overlapping.insert((layer.clone(), level));
            }
            out.push((layer, level, chosen));
        }
        // What a whole-layer response costs: the sum of the per-level counts above, reported and
        // never refused, since a large response is slow, not wrong, and discloses nothing the
        // mask did not already allow.
        for (layer, mut levels) in per_layer {
            levels.sort_unstable();
            let total: u64 = levels.iter().map(|(_, n)| *n).sum();
            tracing::info!(
                layer = %layer,
                evaluated_artifacts = total,
                per_level = ?levels,
                "the fold re-evaluated these levels of a layer; the sum is what a response naming \
                 them carries, one row per served artifact"
            );
        }
        (out, overlapping)
    }

    /// Write this prefix's columns, one file per `(view, layer, level)`: a row-major level's column,
    /// and for a level served artifact-major a label column where its memberships partition the
    /// rows (`mosaica_store::derived::level_column`). Every label column is copied into the order of
    /// the fold's segment's bands beside it. A row-major level whose column will not compose gets no
    /// file, and is served artifact-major instead (see `ArtifactProjections::get_or_build`).
    fn write_row_columns(
        &self,
        pass: &mut DerivedPass<'_>,
        layouts: &[(String, u32, mosaica_types::layer::ServingLayout)],
        overlapping: &Overlapping,
    ) -> Vec<mosaica_store::manifest::DerivedExtent> {
        let prefix_dir = pass.prefix_dir;
        let n = pass.n;
        let incarnations = &pass.incarnations;
        let spaces = &pass.spaces;
        let pending = pass.pending;
        let fold_segments = &pass.segments;
        // Each wanted level, and whether it is spatial. An attribute level has no stored
        // memberships to compose a column from.
        let wanted: Vec<(ChosenLayout, bool)> = layouts
            .iter()
            .filter(|(layer, level, _)| self.composes_row_structures(pending, layer, *level))
            .filter_map(|chosen| {
                let membership = self
                    .live
                    .registered_layer(&chosen.0)?
                    .declaration
                    .membership;
                match membership {
                    mosaica_types::layer::MembershipSource::Enumerated => {
                        Some((chosen.clone(), false))
                    }
                    mosaica_types::layer::MembershipSource::Spatial => Some((chosen.clone(), true)),
                    _ => None,
                }
            })
            .collect();
        if wanted.is_empty() || spaces.is_empty() {
            return Vec::new();
        }

        let scratch = self.projections.scratch();
        let shard = pass.layout.shard;
        let mut written: Vec<(
            String,
            String,
            u32,
            u64,
            mosaica_store::manifest::DerivedForm,
            std::path::PathBuf,
        )> = Vec::with_capacity(wanted.len() * spaces.len());
        // The store's lock is held to read each level alone, and released before the level is
        // projected and its files written, so a write waits for the read and no more.
        for ((layer, level, layout), spatial) in &wanted {
            let partitions = !overlapping.contains(&(layer.clone(), *level));
            let Some((composed_as, form)) =
                mosaica_store::derived::level_column(*layout, partitions)
            else {
                continue;
            };
            let read = self.live.with_artifacts(|store| {
                let version = pending.version_after(store, shard, layer, *level)?;
                let ordinals = level_length(pending.records(store, layer, *level));
                let members = match spatial {
                    true => Vec::new(),
                    false => held_members(store, pending, shard, layer, *level),
                };
                Some((version, ordinals, members))
            });
            let Some((version, ordinals, members)) = read else {
                continue;
            };
            for (view, space) in spaces {
                let segment = fold_segments
                    .iter()
                    .find(|(v, _)| v == view)
                    .map(|(_, segment)| segment);
                let composed = if *spatial {
                    // The fold's segment is the whole base at row base 0, so the piece staged
                    // in `choose_layouts` is the level's membership in this view.
                    let piece = self.shapes.get(view, layer, *level).and_then(|held| {
                        segment.and_then(|segment| held.staged(shard, &segment.seg_id))
                    });
                    match piece {
                        Some(piece) => {
                            let rows = piece.as_ref();
                            mosaica_store::derived::project_row_column(
                                rows.len() as u32,
                                space.base_rows(),
                                composed_as,
                                scratch,
                                &|visit| {
                                    for (ordinal, rows) in rows.iter().enumerate() {
                                        if let Some(rows) = rows {
                                            visit(ordinal as u32, rows);
                                        }
                                    }
                                },
                            )
                        }
                        None => Ok(None),
                    }
                } else {
                    mosaica_store::derived::project_row_column(
                        ordinals,
                        space.base_rows(),
                        composed_as,
                        scratch,
                        &|visit| {
                            for (ordinal, members) in &members {
                                visit(
                                    *ordinal,
                                    &members.projected(|part| space.project_base(part)),
                                );
                            }
                        },
                    )
                };
                match composed {
                    Ok(Some(path)) => {
                        // A served column is filed only with its members beside it.
                        if matches!(form, mosaica_store::manifest::DerivedForm::RowColumn { .. }) {
                            match mosaica_store::derived::stage_row_members(
                                &path,
                                composed_as,
                                scratch,
                            ) {
                                Ok(members) => written.push((
                                    view.clone(),
                                    layer.clone(),
                                    *level,
                                    version,
                                    mosaica_store::manifest::DerivedForm::RowMembers,
                                    members,
                                )),
                                Err(error) => {
                                    tracing::warn!(
                                        layer = %layer,
                                        level,
                                        view = %view,
                                        %error,
                                        "a row-major column's members would not be written \
                                         at the fold; that level derives both on first use"
                                    );
                                    let _ = std::fs::remove_file(&path);
                                    continue;
                                }
                            }
                        }
                        let labelled = segment.filter(|_| {
                            composed_as == mosaica_types::layer::ServingLayout::RowMajorLabel
                        });
                        if let Some(segment) = labelled {
                            match mosaica_store::derived::stage_band_labels(
                                &path,
                                &segment.bands,
                                scratch,
                            ) {
                                Ok(copy) => written.push((
                                    view.clone(),
                                    layer.clone(),
                                    *level,
                                    version,
                                    mosaica_store::manifest::DerivedForm::BandLabels {
                                        seg_id: segment.seg_id.clone(),
                                    },
                                    copy,
                                )),
                                Err(error) => tracing::warn!(
                                    layer = %layer,
                                    level,
                                    view = %view,
                                    %error,
                                    "a level's labels would not be copied into the order of \
                                     its segment's bands; the level has no copy"
                                ),
                            }
                        }
                        written.push((
                            view.clone(),
                            layer.clone(),
                            *level,
                            version,
                            form.clone(),
                            path,
                        ))
                    }
                    // A level served artifact-major whose memberships overlap has no label
                    // column, and that is its shape rather than a fault.
                    Ok(None) if !layout.is_row_major() => {}
                    Ok(None) => tracing::warn!(
                        layer = %layer,
                        level,
                        view = %view,
                        layout = ?layout,
                        "a row-major column would not compose at the fold: this level's \
                         memberships do not partition, so it is served artifact-major. \
                         Every answer is unchanged; the layout is not"
                    ),
                    Err(error) => tracing::warn!(
                        layer = %layer,
                        level,
                        view = %view,
                        %error,
                        form = form.dir(),
                        "a level's column would not be composed at the fold; that level \
                         derives it on first use"
                    ),
                }
            }
        }
        if written.is_empty() {
            return Vec::new();
        }

        mosaica_store::derived::file_derived(
            prefix_dir,
            pass.layout,
            n,
            &mut pass.index,
            written
                .into_iter()
                .filter_map(|(view, layer, level, level_version, form, path)| {
                    // No incarnation, no file: an unstamped structure would label another
                    // view's rows.
                    let Some(incarnation) = incarnations.get(&view).copied() else {
                        let _ = std::fs::remove_file(&path);
                        return None;
                    };
                    let Some(space) = self.live.space_of(&layer) else {
                        let _ = std::fs::remove_file(&path);
                        return None;
                    };
                    Some(mosaica_store::derived::Filed {
                        view: Some(view),
                        incarnation: Some(incarnation),
                        space,
                        level,
                        level_version: self.live.with_artifacts(|store| {
                            pending.records_version_after(store, &layer, level)
                        }),
                        slice_version: level_version,
                        form,
                        bytes: mosaica_store::derived::FiledBytes::Staged(path),
                    })
                })
                .collect(),
        )
    }

    /// Write this prefix's shape row forms: for every spatial level `columns` does not cover, the
    /// piece the fold resolved for each of its segments in `choose_layouts`, keyed by that segment
    /// and the level's version.
    ///
    /// Every failure is a dropped entry, not a discarded fold: the form is derived, and an open
    /// that finds no entry resolves the segment again.
    fn write_shape_rows(
        &self,
        pass: &mut DerivedPass<'_>,
        columns: &[mosaica_store::manifest::DerivedExtent],
    ) -> Vec<mosaica_store::manifest::DerivedExtent> {
        let prefix_dir = pass.prefix_dir;
        let layout = pass.layout;
        let n = pass.n;
        let incarnations = &pass.incarnations;
        let pending = pass.pending;
        let fold_segments = &pass.segments;
        let mut levels: Vec<(String, u32)> = self.live.with_artifacts(|store| {
            levels_of(store)
                .filter(|(layer, level)| !pending.is_pending(layer, *level))
                .collect()
        });
        levels.retain(|(layer, _)| self.live.spatial_layer(layer));
        let mut filed: Vec<mosaica_store::derived::Filed> = Vec::new();
        for (layer, level) in &levels {
            let Some(space) = self.live.space_of(layer) else {
                continue;
            };
            let (records_version, version) = self.live.with_artifacts(|store| {
                (
                    store.records_version(layer, *level),
                    slice_of(store, layout.shard).level_version(layer, *level),
                )
            });
            for (view, segment) in fold_segments {
                let covered = columns.iter().any(|c| {
                    matches!(
                        c.form,
                        mosaica_store::manifest::DerivedForm::RowColumn { .. }
                    ) && c.space == space
                        && c.level == *level
                        && c.view.as_ref() == Some(view)
                });
                if covered {
                    continue;
                }
                let Some(piece) = self
                    .shapes
                    .get(view, layer, *level)
                    .and_then(|held| held.staged(layout.shard, &segment.seg_id))
                else {
                    tracing::warn!(
                        layer = %layer,
                        level,
                        view = %view,
                        seg_id = %segment.seg_id,
                        "the fold holds no resolved piece for this segment, so no row form is \
                         written; the next open resolves it"
                    );
                    continue;
                };
                // No incarnation, no file; see `write_row_columns`.
                let Some(incarnation) = incarnations.get(view).copied() else {
                    continue;
                };
                filed.push(mosaica_store::derived::Filed {
                    view: Some(view.clone()),
                    incarnation: Some(incarnation),
                    space,
                    level: *level,
                    level_version: records_version,
                    slice_version: version,
                    form: mosaica_store::manifest::DerivedForm::ShapeRows {
                        seg_id: segment.seg_id.clone(),
                        row_count: segment.row_count,
                    },
                    bytes: mosaica_store::derived::FiledBytes::InHand(
                        mosaica_store::derived::shape_rows_bytes(
                            version,
                            &segment.seg_id,
                            segment.row_count,
                            &piece,
                        ),
                    ),
                });
            }
        }
        mosaica_store::derived::file_derived(prefix_dir, layout, n, &mut pass.index, filed)
    }

    /// Write this prefix's persisted decompositions: every spatial level's held shapes for each
    /// fold view, as the level holds them at its current version.
    fn write_shape_held(
        &self,
        pass: &mut DerivedPass<'_>,
    ) -> Vec<mosaica_store::manifest::DerivedExtent> {
        let prefix_dir = pass.prefix_dir;
        let layout = pass.layout;
        let n = pass.n;
        let incarnations = &pass.incarnations;
        let pending = pass.pending;
        let fold_segments = &pass.segments;
        let levels: Vec<(String, u32, LayerSpace)> = self
            .live
            .with_artifacts(|store| levels_of(store).collect::<Vec<_>>())
            .into_iter()
            .filter(|(layer, level)| {
                !pending.is_pending(layer, *level) && self.live.spatial_layer(layer)
            })
            .filter_map(|(layer, level)| {
                let space = self.live.space_of(&layer)?;
                Some((layer, level, space))
            })
            .collect();
        let filed: Vec<mosaica_store::derived::Filed> = self.live.with_artifacts(|store| {
            let mut out = Vec::new();
            for (layer, level, space) in &levels {
                let space = *space;
                let version = store.level_version(layer, *level);
                for (view, _) in fold_segments {
                    let Some(held) = self.shapes.get(view, layer, *level) else {
                        continue;
                    };
                    if held.level_version != version {
                        continue;
                    }
                    let shapes: Vec<(Option<&[u8]>, Option<&mosaica_store::derived::HeldShape>)> =
                        (0..held.shapes.len() as u32)
                            .map(|ordinal| {
                                (
                                    store
                                        .shape_of(layer, *level, ordinal)
                                        .and_then(|shapes| shapes.for_view(view)),
                                    held.shapes[ordinal as usize].as_ref(),
                                )
                            })
                            .collect();
                    let Some(incarnation) = incarnations.get(view).copied() else {
                        continue;
                    };
                    out.push(mosaica_store::derived::Filed {
                        view: Some(view.clone()),
                        incarnation: Some(incarnation),
                        space,
                        level: *level,
                        level_version: store.records_version(layer, *level),
                        slice_version: version,
                        form: mosaica_store::manifest::DerivedForm::ShapeHeld,
                        bytes: mosaica_store::derived::FiledBytes::InHand(
                            mosaica_store::derived::shape_held_bytes(version, &shapes),
                        ),
                    });
                }
            }
            out
        });
        mosaica_store::derived::file_derived(prefix_dir, layout, n, &mut pass.index, filed)
    }

    /// Project and write this prefix's tile-index extent columns, one file per
    /// `(view, layer, level)` whose layout is not row-major and whose membership is not spatial.
    fn write_tile_indexes(
        &self,
        pass: &mut DerivedPass<'_>,
        layouts: &[(String, u32, mosaica_types::layer::ServingLayout)],
    ) -> Vec<mosaica_store::manifest::DerivedExtent> {
        let prefix_dir = pass.prefix_dir;
        let n = pass.n;
        let incarnations = &pass.incarnations;
        let spaces = &pass.spaces;
        let pending = pass.pending;
        if spaces.is_empty() {
            return Vec::new();
        }

        let mut levels: Vec<(String, u32)> =
            self.live.with_artifacts(|store| levels_of(store).collect());
        levels.retain(|(layer, level)| {
            self.composes_row_structures(pending, layer, *level)
                && !layouts
                    .iter()
                    .any(|(l, v, layout)| l == layer && v == level && layout.is_row_major())
                && !self.live.registered_layer(layer).is_some_and(|registered| {
                    registered.declaration.membership
                        == mosaica_types::layer::MembershipSource::Spatial
                })
        });
        // The store's lock is taken one level at a time, so a reader waits for one level at most.
        let mut projected: Vec<(String, String, u32, u64, Vec<u8>)> =
            Vec::with_capacity(levels.len() * spaces.len());
        for (layer, level) in &levels {
            self.live.with_artifacts(|store| {
                let shard = pass.layout.shard;
                let Some(version) = pending.version_after(store, shard, layer, *level) else {
                    return;
                };
                let ordinals = level_length(pending.records(store, layer, *level));
                for (view, space) in spaces {
                    let index = crate::tile_index::TileIndex::project(
                        ordinals,
                        || pending.held(store, shard, layer, *level),
                        space,
                        slice_of(store, shard),
                    );
                    projected.push((
                        view.clone(),
                        layer.clone(),
                        *level,
                        version,
                        index.as_bytes().to_vec(),
                    ));
                }
            });
        }
        if projected.is_empty() {
            return Vec::new();
        }

        mosaica_store::derived::file_derived(
            prefix_dir,
            pass.layout,
            n,
            &mut pass.index,
            projected
                .into_iter()
                .filter_map(|(view, layer, level, level_version, bytes)| {
                    let incarnation = *incarnations.get(&view)?;
                    Some(mosaica_store::derived::Filed {
                        view: Some(view),
                        incarnation: Some(incarnation),
                        space: self.live.space_of(&layer)?,
                        level,
                        level_version: self.live.with_artifacts(|store| {
                            pending.records_version_after(store, &layer, level)
                        }),
                        slice_version: level_version,
                        form: mosaica_store::manifest::DerivedForm::TileIndex,
                        bytes: mosaica_store::derived::FiledBytes::InHand(bytes),
                    })
                })
                .collect(),
        )
    }
}

impl Executor {
    /// Rebuild every level's row-space membership, and every lineage this fold moved, against the
    /// live generation.
    ///
    /// Called synchronously at the fold's own publication: the alternative is not a cache miss but
    /// a stall on whichever request arrives first. A view the generation does not carry is simply
    /// not warmed; its first request builds what it needs.
    pub(super) fn warm_artifact_caches(&self) {
        let generation = self.generation.load_full();
        let levels: Vec<(String, u32)> =
            self.live.with_artifacts(|store| levels_of(store).collect());
        if levels.is_empty() {
            return;
        }
        let started = std::time::Instant::now();
        let before_projections = self.deps.artifact_projections.builds();
        let before_lineages = self.deps.lineages.builds();
        for shard in generation.shards() {
            for (view, view_data) in &shard.data.views {
                for (layer, level) in &levels {
                    // The layout the fold has just recorded, so the warm builds the form the next
                    // request will ask for rather than one it would immediately replace.
                    let layout = self
                        .live
                        .registered_layer(layer)
                        .map(|registered| registered.layout_of(*level))
                        .unwrap_or_default();
                    // A predicate level is not warmed here: it is evaluated against the geometry
                    // by the request path, not assembled from a stored membership.
                    let registered = self.live.registered_layer(layer);
                    let membership = registered
                        .as_ref()
                        .map(|r| r.declaration.membership.clone());
                    let spatial = registered
                        .as_ref()
                        .is_some_and(|r| spatial_membership(&r.declaration));
                    if matches!(
                        membership,
                        Some(mosaica_types::layer::MembershipSource::Attribute(_))
                    ) || (!spatial
                        && !matches!(
                            membership,
                            Some(mosaica_types::layer::MembershipSource::Enumerated)
                        ))
                    {
                        continue;
                    }
                    let Some(registered) = registered.as_ref() else {
                        continue;
                    };
                    let segments = if spatial {
                        crate::viewport::segments_with_row_bases(view, view_data).ok()
                    } else {
                        None
                    };
                    self.live.with_artifacts(|store| {
                        let predicate = segments.as_ref().map(|segments| {
                            crate::artifacts::PredicateSource::Spatial(
                                crate::artifacts::SpatialSource {
                                    level: self.deps.shapes.level(
                                        view,
                                        view_data.incarnation,
                                        layer,
                                        *level,
                                        store,
                                        &crate::shapes::PersistedPieces::none(),
                                    ),
                                    segments,
                                    total_rows: u32::try_from(view_data.row_space.total_rows())
                                        .unwrap_or(u32::MAX),
                                },
                            )
                        });
                        self.deps.artifact_projections.get_or_build(
                            &generation.prefix,
                            shard.id(),
                            view,
                            view_data.incarnation,
                            layer,
                            *level,
                            slice_of(store, shard.id()),
                            store,
                            &view_data.row_space,
                            Some(&shard.partition_source()),
                            layout,
                            predicate.as_ref(),
                            shard.segments_version,
                            &registered.declaration,
                        )
                    });
                }
            }
        }
        for (layer, level) in &levels {
            self.live.with_artifacts(|store| {
                self.deps.lineages.get_or_build(
                    layer,
                    *level,
                    crate::cut::LineageAt::current(store.lineage_version(layer, *level)),
                    || {
                        crate::cut::Lineage::new(store.level(layer, *level).map(
                            |(ordinal, record)| {
                                let within = record
                                    .parents
                                    .iter()
                                    .find(|parent| parent.level == *level)
                                    .map(|parent| parent.ordinal);
                                (ordinal, within)
                            },
                        ))
                    },
                )
            });
        }
        tracing::info!(
            projections = self.deps.artifact_projections.builds() - before_projections,
            lineages = self.deps.lineages.builds() - before_lineages,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "the fold's artifact pass rebuilt every level's row form"
        );
    }
}
/// The segments a fold wrote, opened from the prefix it wrote them into: what its artifact pass
/// resolves every spatial level against. A segment that will not open is skipped and logged; the
/// level then resolves it at the flip's warm instead, on the executor.
pub(super) fn fold_segments(
    prefix_dir: &std::path::Path,
    layout: ShardLayout<'_>,
    segments: &[mosaica_store::manifest::SegmentDescriptor],
) -> Vec<(String, Arc<mosaica_store::read::SegmentData>)> {
    let mut out = Vec::with_capacity(segments.len());
    for descriptor in segments {
        let dir = prefix_dir.join(layout.segment_rel(&descriptor.view, &descriptor.seg_id));
        // A fold writes each view's base, whose rows name their entities in the view's table.
        let table_path = prefix_dir
            .join(layout.view_rel(&descriptor.view))
            .join(mosaica_store::ROW_ENTITY_FILE);
        let table = table_path
            .exists()
            .then(|| mosaica_store::RowToEntity::load(&table_path));
        let entities = match table {
            Some(Ok(table)) => {
                mosaica_store::edited::RowEntities::Table(std::sync::Arc::new(table))
            }
            Some(Err(e)) => {
                tracing::warn!(
                    view = %descriptor.view,
                    error = %e,
                    "the fold could not reopen a row-to-entity file it just wrote; its spatial \
                     memberships are resolved at the flip instead"
                );
                continue;
            }
            None => mosaica_store::edited::RowEntities::Numbers,
        };
        match mosaica_store::read::SegmentData::load(
            &dir,
            &descriptor.seg_id,
            descriptor.row_count,
            entities,
        ) {
            Ok(segment) => out.push((descriptor.view.clone(), Arc::new(segment))),
            Err(e) => tracing::warn!(
                view = %descriptor.view,
                seg_id = %descriptor.seg_id,
                error = %e.source,
                "the fold could not reopen a segment it just wrote; its spatial memberships are \
                 resolved at the flip instead"
            ),
        }
    }
    out
}

/// One superseded prefix awaiting reclamation, and the generations published over it, whose
/// release says no thread can still resolve a path inside it. See [`Executor::superseded`].
pub(in crate::write) struct PendingReclaim {
    generations: Vec<std::sync::Weak<Generation>>,
    prefix_dir: PathBuf,
}

/// Seconds since the Unix epoch, or `None` if the clock is before it.
///
/// `None` reads as "no fold has ended yet", switching the interval floor off rather than jamming it
/// on: the safe direction, and the same answer a fresh process gives.
pub(in crate::write) fn unix_now() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_secs())
}
