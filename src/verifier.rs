use crate::hasher::DirectoryHasher;
use crate::manifest::Manifest;
use crate::util::{CancelHandle, Error};

#[derive(bon::Builder)]
pub struct DirectoryVerifier<'a> {
    hasher: DirectoryHasher,

    _manifest: &'a Manifest,
}

impl<'a> DirectoryVerifier<'a> {
    /// Calls [`DirectoryHasher::cancel_handle`] on the internal [`DirectoryHasher`].
    pub fn cancel_handle(&mut self) -> CancelHandle {
        self.hasher.cancel_handle()
    }

    pub fn verify(&mut self) -> Result<(), Error> {
        todo!()
    }
}

// /// Determines (in parallel) the set difference between `slice_1` and `slice_2`.
// ///
// /// Effectively: `result` = `slice_1` - `slice_2`.
// fn find_difference(slice_1: &[Entry], slice_2: &[Entry]) -> Vec<Entry> {
//     slice_1
//         .into_par_iter()
//         .filter(|entry| {
//             slice_2
//                 .binary_search_by(|e| e.path.cmp(&entry.path))
//                 .is_err()
//         })
//         .cloned()
//         .collect()
// }

// pub enum Difference {
//     Old(Entries),
//     New(Entries),
//     OldAndNew((Entries, Entries)),
//     None,
// }

// impl Manifest {
//     pub fn cancel_handle(&mut self) -> CancelHandle {
//         self.directory_hasher.cancel_handle()
//     }

//     pub fn progress_channel(&mut self, sender: Sender<Event>) {
//         self.directory_hasher.progress_channel = Some(sender);
//     }

//     #[inline(never)]
//     pub fn verify(&self) -> Result<Difference, Error> {
//         let old_entries = &self.entries;
//         let new_entries = self.directory_hasher.hash_entries()?;
//         let missing_from_new = find_difference(old_entries, &new_entries);
//         let missing_from_old = find_difference(&new_entries, old_entries);
//         Ok(
//             match (missing_from_new.is_empty(), missing_from_old.is_empty()) {
//                 (true, true) => Difference::None,
//                 (false, true) => Difference::New(missing_from_new.into()),
//                 (true, false) => Difference::Old(missing_from_old.into()),
//                 (false, false) => {
//                     Difference::OldAndNew((missing_from_old.into(), missing_from_new.into()))
//                 }
//             },
//         )
//     }

//     #[inline(never)]
//     pub fn verify_entries(&self, new_entries: Entries) -> Result<Option<Entries>, Error> {
//         _ = new_entries;
//         todo!()
//     }
// }
