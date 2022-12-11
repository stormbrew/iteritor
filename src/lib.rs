/*
 Copyright 2022 The Iteritor Authors

 Licensed under the Apache License, Version 2.0 (the "License");
 you may not use this file except in compliance with the License.
 You may obtain a copy of the License at

      https://www.apache.org/licenses/LICENSE-2.0

 Unless required by applicable law or agreed to in writing, software
 distributed under the License is distributed on an "AS IS" BASIS,
 WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 See the License for the specific language governing permissions and
 limitations under the License.
 */

#![cfg_attr(feature = "nightly", feature(try_trait_v2))]

pub mod breaking;
pub mod buffer;
pub mod filtered;
pub mod try_polyfill;

use breaking::*;
use buffer::*;
use filtered::*;

use try_polyfill::Try;

/// Including this trait adds the `with_filtered` (and `with_filtered_buf` for
/// nostd use) helper function to any normal iterator.
/// 
/// These functions allow you to break out an iterator of wrapped values to 
/// perform some normal standard iterator chaining only on the 'normal' 
/// (ie. `Ok`, `Some`) values and then pull from a recombined
/// iterator that includes both in the same order as they were originally
/// evaluated.
/// 
/// It does this while only storing runs of 'divergent' values (`Err`, `None`)
/// in a deque, avoiding the need to `.collect()` mid-processing to get at 
/// the values in a complicated chain and still use normal combinators.
/// 
#[cfg_attr(feature = "std", doc = r##"
Example:

```
# use iteritor::IterFiltered;
# use iteritor::buffer::Buffer;
let items = [
    Ok(1),
    Ok(2),
    Ok(1),
    Err("boom"),
    Err("hi"),
    Ok(3),
    Ok(1),
    Err("zoop"),
    Ok(5)
];

// With this library
let with: Vec<_> = items
    .into_iter()
    .with_filtered(|filtered| {
        filtered
            .map(|n| n * 2)
            .filter(|n| *n > 3)
            .map(|n| n / 2)
    })
    .collect();

let without: Vec<_> = items
    .into_iter()
    .filter_map(|i| match i {
        Ok(n) => {
            if n * 2 > 3 {
                Some(Ok(n))
            } else {
                None
            }
        },
        Err(err) => Some(Err(err)),
    })
    .collect();

# assert_eq!(with, without)
```
"##)]
pub trait IterFiltered<T>: Iterator {
    /// Takes the `self` iterator and a [`buffer::ControlFlowBuffer`] `buf` and
    /// calls `f` with an iterator that will provide the 'normal' values. 
    /// 
    /// An iterator will be returned that includes, in order, a combination of
    /// the skipped divergent values and the results of the iterator chain in
    /// the function on the normal values.
    /// 
    /// This variant is mostly for use in no std/no alloc code where you can
    /// supply your own (maybe fixed-size) buffer implementation.
    fn with_filtered_buf<B, O, U, F>(self, buf: B, f: F) -> DefilteredIter<O, B>
    where
        Self: Iterator<Item = T> + Sized,
        F: Fn(FilteredIter<Self, B>) -> O,
        O: Iterator<Item = U>,
        T: Try<Output = U>,
        B: ControlFlowBuffer<Item = T>;

    /// Takes the `self` iterator and calls `f` with an iterator that will 
    /// provide the 'normal' values. 
    /// 
    /// An iterator will be returned that includes, in order, a combination of
    /// the skipped divergent values and the results of the iterator chain in
    /// the function on the normal values.
    #[cfg(feature = "std")]
    fn with_filtered<O, U, F>(self, f: F) -> DefilteredIter<O, buffer::Buffer<T>>
    where
        Self: Iterator<Item = T> + Sized,
        F: Fn(FilteredIter<Self, buffer::Buffer<T>>) -> O,
        O: Iterator<Item = U>,
        T: Try<Output = U>,
    {
        self.with_filtered_buf(buffer::Buffer::default(), f)
    }
}

impl<I, T> IterFiltered<T> for I
where
    I: Iterator<Item = T>,
{
    fn with_filtered_buf<B, O, U, F>(self, buffer: B, f: F) -> DefilteredIter<O, B>
    where
        Self: Iterator<Item = T> + Sized,
        F: Fn(FilteredIter<Self, B>) -> O,
        O: Iterator<Item = U>,
        T: Try<Output = U>,
        B: ControlFlowBuffer<Item = T>,
    {
        let original_max = self.size_hint().1;
        let buf_iter = FilteredIter::new(self, buffer.clone());

        DefilteredIter::new(f(buf_iter), buffer, original_max)
    }
}

/// Including this trait adds the `with_folding` helper function to any normal 
/// iterator.
/// 
/// This function allows you to do any fold-style operation (like 
/// [`Iterator::max`], [`Iterator::max_by_key`], [`Iterator::find`], or even
/// [`Iterator::fold`] itself) while breaking on any divergent value (like `Err`
/// or `None`).
/// 
/// This helps avoid the proliferation of various variants of folding functions
/// just to deal specifically with results, and helps errors propagate more
/// easily. The alternative is to collect into a `Result<Vec<_>,_>` first, 
/// which may not be very efficient.
/// 
#[cfg_attr(feature = "std", doc = r##"
Example:

```
# use iteritor::IterFolding;
# use iteritor::buffer::Buffer;
let items = [
    Ok(1),
    Ok(2),
    Ok(1),
    Err("boom"),
    Err("hi"),
    Ok(3),
    Ok(1),
    Err("zoop"),
    Ok(5)
];

let with = items
    .into_iter()
    .with_folding(|i| i.sum::<u32>());

let without = items
    .into_iter()
    .collect::<Result<Vec<_>,_>>()
    .map(|v|
        v.into_iter().sum::<u32>()
    );

# assert_eq!(with, without)
```
"##)]
pub trait IterFolding<T> {
    /// Takes the `self` iterator and calls `f` with an iterator that will 
    /// provide the 'normal' values, returning either the first encountered
    /// failure value or the result of the function.
    fn with_folding<O, F, R>(self, f: F) -> T
    where
        Self: Iterator<Item = T> + Sized,
        F: Fn(BreakingIterator<Self, T::Residual>) -> O,
        T: Try<Output = O, Residual = R>;
}

impl<I, T> IterFolding<T> for I
where
    I: Iterator<Item = T>,
{
    fn with_folding<O, F, R>(self, f: F) -> T
    where
        Self: Iterator<Item = T> + Sized,
        F: Fn(BreakingIterator<Self, T::Residual>) -> O,
        T: Try<Output = O, Residual = R>,
    {
        let mut result = None;
        let buf_iter = BreakingIterator::new(self, &mut result);

        let ok_res = f(buf_iter);

        // if anything is in the output buffer it *should* be an error.
        if let Some(err) = result {
            T::from_residual(err)
        } else {
            T::from_output(ok_res)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use filtered::tests::result_samples;

    #[cfg(feature = "std")]
    #[test]
    fn filtered() {
        let mut output = result_samples().with_filtered(|i| i.filter(|n| *n > 2));

        assert_eq!(output.next(), Some(Err("boom")));
        assert_eq!(output.next(), Some(Err("hi")));
        assert_eq!(output.next(), Some(Ok(3)));
        assert_eq!(output.next(), Some(Err("zoop")));
        assert_eq!(output.next(), Some(Ok(5)));
        assert_eq!(output.next(), None);
    }

    #[test]
    fn folded() {
        result_samples()
            .with_folding(|i| i.sum::<u32>())
            .expect_err("foldable sum of sample data should error");
        assert_eq!(
            result_samples()
                .take(3)
                .with_folding(|i| i.sum::<u32>())
                .unwrap(),
            4
        );
    }
}
