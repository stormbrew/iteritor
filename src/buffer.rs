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

use core::ops::ControlFlow;

use crate::try_polyfill::{FromResidual, Try};

pub trait ControlFlowBuffer: Default + Clone {
    type Item;

    /// Add an item to the buffer queue
    fn push(&self, input: Self::Item);
    /// Remove an item from the buffer queue
    fn pop(&self) -> Option<Self::Item>;

    /// Both push and pop with an optimization that we don't
    /// bother putting it in the queue if we'd just be popping
    /// this value off anyways.
    fn push_and_pop(&self, input: Self::Item) -> Self::Item {
        if let Some(next) = self.pop() {
            self.push(input);
            next
        } else {
            input
        }
    }

    /// Advances the given iterator until an item that unwraps is found,
    /// and returns that. Any items found in the meantime will be added to
    /// the buffer. This returns None when no more items are available
    /// (As with [`Iterator::next`]).
    fn next_unwrapped<O>(&self, iter: &mut impl Iterator<Item = Self::Item>) -> Option<O>
    where
        Self::Item: Try<Output = O>,
    {
        for next in iter {
            use ControlFlow::*;
            match next.branch() {
                Continue(next) => return Some(next),
                Break(residual) => self.push(FromResidual::from_residual(residual)),
            }
        }
        None
    }

    /// Returns the next item out of the buffer, or if there aren't any,
    /// advances the given iterator and returns either the first item buffered
    /// as a result of that or the item returned from the iterator if none
    /// were buffered.
    fn next_wrapped<O>(&self, iter: &mut impl Iterator<Item = O>) -> Option<Self::Item>
    where
        Self::Item: Try<Output = O>,
    {
        if let Some(next) = self.pop() {
            Some(next)
        } else {
            iter.next()
                .map(|next| self.push_and_pop(Self::Item::from_output(next)))
        }
    }
}

#[cfg(feature = "std")]
mod is_std {
    use super::*;

    use core::cell::RefCell;
    use std::{collections::VecDeque, rc::Rc};

    pub type Buffer<T> = Rc<RefCell<VecDeque<T>>>;

    impl<T> ControlFlowBuffer for Rc<RefCell<VecDeque<T>>> {
        type Item = T;

        fn push(&self, input: T) {
            self.borrow_mut().push_back(input)
        }
        fn pop(&self) -> Option<T> {
            self.borrow_mut().pop_front()
        }
    }
}

#[cfg(not(feature = "std"))]
mod is_std {}

pub use is_std::*;
