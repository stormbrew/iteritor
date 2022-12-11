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

use crate::try_polyfill::Try;
use crate::ControlFlowBuffer;

pub struct FilteredIter<Input, Buffer> {
    input_iter: Input,
    buffer: Buffer,
}

impl<I, B, T> FilteredIter<I, B>
where
    I: Iterator<Item = T>,
{
    pub fn new(input_iter: I, buffer: B) -> Self {
        Self { input_iter, buffer }
    }
}

impl<I, B, T> Iterator for FilteredIter<I, B>
where
    I: Iterator<Item = T>,
    T: Try,
    B: ControlFlowBuffer<Item = T>,
{
    type Item = T::Output;
    fn next(&mut self) -> Option<T::Output> {
        self.buffer.next_unwrapped(&mut self.input_iter)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        // since this is a filtering iterator, we may give out
        // fewer than our input iterator puts out.
        let (_, high) = self.input_iter.size_hint();
        (0, high)
    }
}

pub struct DefilteredIter<Input, Buffer> {
    input_iter: Input,
    buffer: Buffer,
    original_max: Option<usize>,
}

impl<I, B, T> DefilteredIter<I, B>
where
    I: Iterator<Item = T::Output>,
    T: Try,
    B: ControlFlowBuffer<Item = T>,
{
    pub fn new(input_iter: I, buffer: B, original_max: Option<usize>) -> Self {
        Self {
            input_iter,
            buffer,
            original_max,
        }
    }
}

impl<I, B, T> Iterator for DefilteredIter<I, B>
where
    I: Iterator<Item = T::Output>,
    T: Try,
    B: ControlFlowBuffer<Item = T>,
{
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.buffer.next_wrapped(&mut self.input_iter)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        // out max output size is the bigger of the original iterator's
        // maximum size (since we're adding things back in) or our input
        // iterator's max size (since it might be doing something like
        // duping each output)
        let max_size = [self.input_iter.size_hint().1, self.original_max]
            .into_iter()
            .flatten()
            .max();
        (0, max_size)
    }
}

#[cfg(test)]
pub mod tests {
    pub fn result_samples() -> impl Iterator<Item = Result<u32, &'static str>> {
        [
            Ok(1),
            Ok(2),
            Ok(1),
            Err("boom"),
            Err("hi"),
            Ok(3),
            Ok(1),
            Err("zoop"),
            Ok(5),
        ]
        .into_iter()
    }

    #[cfg(feature = "std")]
    mod is_std {
        use super::{super::*, *};

        use crate::Buffer;

        #[test]
        fn result_unwrapped_gives_all_ok_items() {
            let buf = Buffer::default();
            let mut samples = result_samples();

            assert_eq!(buf.next_unwrapped(&mut samples), Some(1));
            assert_eq!(buf.next_unwrapped(&mut samples), Some(2));
            assert_eq!(buf.next_unwrapped(&mut samples), Some(1));
            assert_eq!(buf.next_unwrapped(&mut samples), Some(3));
            assert_eq!(buf.next_unwrapped(&mut samples), Some(1));
            assert_eq!(buf.next_unwrapped(&mut samples), Some(5));
            assert_eq!(buf.next_unwrapped(&mut samples), None);
        }

        #[test]
        fn result_wrapped_gives_all_items() {
            let buf = Buffer::default();
            let mut samples = result_samples();
            let mut filtered = std::iter::from_fn(|| buf.next_unwrapped(&mut samples));

            assert_eq!(buf.next_wrapped(&mut filtered), Some(Ok(1)));
            assert_eq!(buf.next_wrapped(&mut filtered), Some(Ok(2)));
            assert_eq!(buf.next_wrapped(&mut filtered), Some(Ok(1)));
            assert_eq!(buf.next_wrapped(&mut filtered), Some(Err("boom")));
            assert_eq!(buf.next_wrapped(&mut filtered), Some(Err("hi")));
            assert_eq!(buf.next_wrapped(&mut filtered), Some(Ok(3)));
            assert_eq!(buf.next_wrapped(&mut filtered), Some(Ok(1)));
            assert_eq!(buf.next_wrapped(&mut filtered), Some(Err("zoop")));
            assert_eq!(buf.next_wrapped(&mut filtered), Some(Ok(5)));
            assert_eq!(buf.next_wrapped(&mut filtered), None);
        }
    }
}
