use crate::prelude::*;

//TODO - move this to dominator-helpers
pub fn enumerate_signal<S, T>(signal: S) -> impl SignalVec<Item = (T, usize)>
where
    S: SignalVec<Item = T>,
    T: Clone,
{
    signal
        .enumerate()
        .map_signal(|(index, data)| {
            index
                .signal()
                .map(move |index| index.map(clone!(data => move |index| (data, index))))
        })
        .filter_map(|item| item)
}
