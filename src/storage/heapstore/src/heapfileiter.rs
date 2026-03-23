use crate::heap_page::HeapPageIntoIter;
use crate::heapfile::HeapFile;
use common::prelude::*;
use std::iter::Peekable;
use std::sync::Arc;

#[allow(dead_code)]
/// The struct for a HeapFileIterator.
/// We use a slightly different approach for HeapFileIterator than
/// standard way of Rust's IntoIter for simplicity (avoiding lifetime issues).
/// This should store the state/metadata required to iterate through the file.
///
/// HINT: This will need an Arc<HeapFile>
pub struct HeapFileIterator {
    hf: Arc<HeapFile>,
    tid: TransactionId,
    current_page_id: PageId,
    page_iter: Option<Peekable<HeapPageIntoIter>>,
}

/// Required HeapFileIterator functions
impl HeapFileIterator {
    /// Create a new HeapFileIterator that stores the tid, and heapFile pointer.
    /// This should initialize the state required to iterate through the heap file.
    pub(crate) fn new(tid: TransactionId, hf: Arc<HeapFile>) -> Self {
        HeapFileIterator {
            hf,
            tid,
            current_page_id: 0,
            page_iter: None,
        }
    }

    pub(crate) fn new_from(tid: TransactionId, hf: Arc<HeapFile>, value_id: ValueId) -> Self {
        let start_page = value_id.page_id.unwrap_or(0);
        let start_slot = value_id.slot_id.unwrap_or(0);
        let page_iter = hf.read_page_from_file(start_page).ok().map(|page| {
            let mut iter = page.into_iter().peekable();
            while iter.peek().map_or(false, |(_, slot)| *slot < start_slot) {
                iter.next();
            }
            iter
        });
        HeapFileIterator {
            hf,
            tid,
            current_page_id: start_page + 1,
            page_iter,
        }
    }
}

/// Trait implementation for heap file iterator.
/// Note this will need to iterate through the pages and their respective iterators.
impl Iterator for HeapFileIterator {
    type Item = (Vec<u8>, ValueId);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref mut iter) = self.page_iter {
                if let Some((bytes, slot_id)) = iter.next() {
                    let page_id = self.current_page_id - 1;
                    let value_id = ValueId::new_slot(self.hf.container_id, page_id, slot_id);
                    return Some((bytes, value_id));
                }
            }
            if self.current_page_id >= self.hf.num_pages() {
                return None;
            }
            let page = self.hf.read_page_from_file(self.current_page_id).ok()?;
            self.current_page_id += 1;
            self.page_iter = Some(page.into_iter().peekable());
        }
    }
}
