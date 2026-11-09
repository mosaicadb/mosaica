//! The one merge over term-sorted sources, which writes every term index and keyword dictionary
//! that is made by merging: the build's runs and its accumulators' remainders, the flush's, and
//! the fold's and the coalesce's finished layers.
//!
//! A [`TermSource`] yields its terms ascending, each with how many entities it holds for the term
//! and then those entities ascending, each with the term's positions in it. Three kinds implement
//! it: a spilled run ([`crate::TextRunWriter`]'s file, read by [`crate::TextRunReader`]), what an
//! accumulator held when its pass ended (which [`crate::TermAccumulator::finish`] hands back), and
//! a finished layer, read through the one [`DictCursor`] both families share. [`TermMerge`] merges
//! any number of them, of any kind, k-way on the term, and each selected term's entities by
//! comparing the sources' heads, because two sources need not partition entity space.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::io;

use croaring::Bitmap;
use mosaica_filter::SortedDict;

/// One term-sorted input to a [`TermMerge`].
pub trait TermSource {
    /// Move to the next term, returning `false` at the end, which a run verifies against its
    /// receipt. A source that has not been moved is before its first term.
    fn next_term(&mut self) -> io::Result<bool>;

    /// The term the source is at.
    fn term(&self) -> &[u8];

    /// How many entities [`Self::next_entity`] yields for the term, known before any is read.
    fn count(&self) -> u64;

    /// The term's next entity, ascending, or `None` once it has yielded [`Self::count`].
    fn next_entity(&mut self) -> io::Result<Option<u32>>;

    /// The term's positions in the entity [`Self::next_entity`] last yielded: empty where the
    /// source carries none.
    fn positions(&self) -> &[u32];

    /// The term's posting and positions records as a finished layer stores them, where this source
    /// yields every entity they hold: what the term is written as when no other source holds it.
    fn records(&self) -> Option<Records<'_>> {
        None
    }
}

/// A term's posting and positions records in a finished layer. The posting was checked when the
/// layer's postings were opened; the positions are checked as a reader decodes them.
#[derive(Clone, Copy)]
pub struct Records<'a> {
    pub posting: &'a [u8],
    pub positions: &'a [u8],
}

/// A k-way merge over term sources, yielding each distinct term once with its entities ascending.
///
/// The heap holds **source indices**, and the comparison reaches into the sources, so a term is
/// never copied into the heap and the merge allocates nothing per term.
pub struct TermMerge<S> {
    sources: Vec<S>,
    heap: Vec<usize>,
    /// The selected term, held because the caller needs it after its sources have moved on.
    term: Vec<u8>,
    /// The sources at the selected term, ascending.
    selected: Vec<usize>,
    /// Entities the selected term carries across them.
    postings: u64,
    /// The selected sources' unconsumed heads, `(entity, source)`, smallest first.
    heads: BinaryHeap<Reverse<(u32, usize)>>,
}

impl<S: TermSource> TermMerge<S> {
    /// A merge over `sources`, each moved to its first term.
    pub fn open(mut sources: Vec<S>) -> io::Result<TermMerge<S>> {
        let mut heap = Vec::with_capacity(sources.len());
        for (i, source) in sources.iter_mut().enumerate() {
            if source.next_term()? {
                heap.push(i);
            }
        }
        for root in (0..heap.len() / 2).rev() {
            sift_down(&mut heap, &sources, root);
        }
        Ok(TermMerge {
            sources,
            heap,
            term: Vec::new(),
            selected: Vec::new(),
            postings: 0,
            heads: BinaryHeap::new(),
        })
    }

    /// Select the next distinct term, or `false` when every source is exhausted. The previous
    /// term's sources are moved on here rather than by the caller, so a caller that took fewer
    /// entities than the term carried still leaves each source where its next term starts.
    pub fn next_term(&mut self) -> io::Result<bool> {
        for &i in &self.selected {
            if self.sources[i].next_term()? {
                push(&mut self.heap, &self.sources, i);
            }
        }
        self.selected.clear();
        self.postings = 0;
        let Some(&first) = self.heap.first() else {
            return Ok(false);
        };
        self.term.clear();
        self.term.extend_from_slice(self.sources[first].term());
        while self
            .heap
            .first()
            .is_some_and(|&top| self.sources[top].term() == self.term.as_slice())
        {
            let top = pop(&mut self.heap, &self.sources);
            self.postings += self.sources[top].count();
            self.selected.push(top);
        }
        Ok(true)
    }

    pub fn term(&self) -> &[u8] {
        &self.term
    }

    /// How many entities the selected term carries, known before any is read, which is what lets
    /// the caller choose how to hold them.
    pub fn postings(&self) -> u64 {
        self.postings
    }

    /// The sources at the selected term, with their indices in the order they were given.
    pub fn selected(&self) -> impl Iterator<Item = (usize, &S)> {
        self.selected.iter().map(|&i| (i, &self.sources[i]))
    }

    /// The selected term's records as one finished layer stores them, where that layer is the only
    /// source holding the term and yields every entity they hold.
    pub fn records(&self) -> Option<Records<'_>> {
        match self.selected.as_slice() {
            [only] => self.sources[*only].records(),
            _ => None,
        }
    }

    /// Feed the selected term's entities to `sink`, ascending, each with the term's positions in
    /// it, checking the ascent as it goes. A failure of the sources reaches the caller through
    /// `lift`, in the sink's error type.
    ///
    /// The check is over the *merged* list, not each source's: it is what catches an entity two
    /// sources carry for one term, which no caller produces (a text entity has one document and one
    /// layer holds it, and a keyword row is visited once) and which a union would otherwise hide.
    pub fn drain<E>(
        &mut self,
        lift: impl Fn(io::Error) -> E,
        sink: &mut impl FnMut(u32, &[u32]) -> Result<(), E>,
    ) -> Result<(), E> {
        self.heads.clear();
        for &i in &self.selected {
            if let Some(entity) = self.sources[i].next_entity().map_err(&lift)? {
                self.heads.push(Reverse((entity, i)));
            }
        }
        let mut last: Option<u32> = None;
        while let Some(Reverse((entity, i))) = self.heads.pop() {
            if last.is_some_and(|previous| entity <= previous) {
                return Err(lift(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "term merge: term {:?}'s entity {entity} does not ascend past {}: two \
                         sources carry the same entity for one term",
                        String::from_utf8_lossy(&self.term),
                        last.unwrap_or_default()
                    ),
                )));
            }
            last = Some(entity);
            sink(entity, self.sources[i].positions())?;
            if let Some(next) = self.sources[i].next_entity().map_err(&lift)? {
                self.heads.push(Reverse((next, i)));
            }
        }
        Ok(())
    }
}

/// Order two sources by their term, ties broken by index so equal terms leave the heap in the
/// order the sources were given.
fn before<S: TermSource>(sources: &[S], a: usize, b: usize) -> bool {
    match sources[a].term().cmp(sources[b].term()) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => a < b,
    }
}

fn sift_down<S: TermSource>(heap: &mut [usize], sources: &[S], mut node: usize) {
    loop {
        let left = node * 2 + 1;
        if left >= heap.len() {
            return;
        }
        let right = left + 1;
        let child = if right < heap.len() && before(sources, heap[right], heap[left]) {
            right
        } else {
            left
        };
        if !before(sources, heap[child], heap[node]) {
            return;
        }
        heap.swap(node, child);
        node = child;
    }
}

fn push<S: TermSource>(heap: &mut Vec<usize>, sources: &[S], i: usize) {
    heap.push(i);
    let mut node = heap.len() - 1;
    while node > 0 {
        let parent = (node - 1) / 2;
        if !before(sources, heap[node], heap[parent]) {
            return;
        }
        heap.swap(node, parent);
        node = parent;
    }
}

fn pop<S: TermSource>(heap: &mut Vec<usize>, sources: &[S]) -> usize {
    let top = heap[0];
    let last = heap.pop().expect("the heap is not empty");
    if !heap.is_empty() {
        heap[0] = last;
        sift_down(heap, sources, 0);
    }
    top
}

/// A finished layer's dictionary as a term source, stepping with [`SortedDict::key_of`]: its
/// keys, or only those whose ordinals `live` holds. A dictionary alone carries no entities, so
/// each key's count is zero; a text layer's source reads the entities from its postings.
///
/// `key_of` re-decodes its block from the restart each time, about half a block of entries a key,
/// and that is what lets several cursors coexist where the push-driven [`SortedDict::walk`] cannot.
/// At the shipped restart interval of 16 that is 8.5 entry decodes a key, which at a measured 11.0
/// to 18.8 ns a decode models to 0.1 to 0.16 µs a key.
pub struct DictCursor<'a> {
    dict: &'a SortedDict,
    live: Option<&'a Bitmap>,
    /// The ordinal whose key `scratch` holds, and the next to look at.
    at: u32,
    next: u32,
    scratch: Vec<u8>,
}

impl<'a> DictCursor<'a> {
    pub fn new(dict: &'a SortedDict, live: Option<&'a Bitmap>) -> DictCursor<'a> {
        DictCursor {
            dict,
            live,
            at: 0,
            next: 0,
            scratch: Vec::new(),
        }
    }

    /// The ordinal of the key the cursor is at.
    pub fn at(&self) -> u32 {
        self.at
    }
}

impl TermSource for DictCursor<'_> {
    fn next_term(&mut self) -> io::Result<bool> {
        while self.next < self.dict.len() {
            let ordinal = self.next;
            self.next += 1;
            if self.live.is_none_or(|live| live.contains(ordinal)) {
                self.dict.key_of(ordinal, &mut self.scratch)?;
                self.at = ordinal;
                return Ok(true);
            }
        }
        self.scratch.clear();
        Ok(false)
    }

    fn term(&self) -> &[u8] {
        &self.scratch
    }

    fn count(&self) -> u64 {
        0
    }

    fn next_entity(&mut self) -> io::Result<Option<u32>> {
        Ok(None)
    }

    fn positions(&self) -> &[u32] {
        &[]
    }
}
