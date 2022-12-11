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

use crate::try_polyfill::Try;

pub struct BreakingIterator<'a, Input, Residual> {
    input_iter: Input,
    result: &'a mut Option<Residual>,
}

impl<'a, I, T, R> BreakingIterator<'a, I, R>
where
    I: Iterator<Item = T>,
    T: Try<Residual = R>,
{
    pub fn new(input_iter: I, result: &'a mut Option<R>) -> Self {
        Self { input_iter, result }
    }
}

impl<'a, I, T, R> Iterator for BreakingIterator<'a, I, R>
where
    I: Iterator<Item = T>,
    T: Try<Residual = R>,
{
    type Item = T::Output;
    fn next(&mut self) -> Option<T::Output> {
        use ControlFlow::*;
        // always return None after we've found a break result
        if self.result.is_none() {
            match self.input_iter.next().map(T::branch)? {
                Continue(next) => Some(next),
                Break(residual) => {
                    self.result.replace(residual);
                    None
                }
            }
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        // since this is a filtering iterator, we may give out
        // fewer than our input iterator puts out.
        let (_, high) = self.input_iter.size_hint();
        (0, high)
    }
}
