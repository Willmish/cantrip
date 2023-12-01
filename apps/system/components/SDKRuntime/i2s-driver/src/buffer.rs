// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A u32 buffer with a beginning and ending that wrap around a fixed size array.
//!
//! This is a FIFO queue that overwrites when the buffer is full.

const BUFFER_CAPACITY: usize = 1024; // XXX to match AUDIO_RECORD_CAPACITY

type ItemType = u32;

#[derive(Debug, PartialEq)]
pub struct Buffer {
    begin: usize,
    end: usize,
    size: usize,
    data: [ItemType; BUFFER_CAPACITY],
}

impl Buffer {
    pub const fn new() -> Buffer {
        Self {
            begin: 0,
            end: 0,
            size: 0,
            data: [0; BUFFER_CAPACITY],
        }
    }

    /// Resets buffer.
    ///
    /// This does not modify the data.
    pub fn clear(&mut self) {
        self.begin = 0;
        self.end = 0;
    }

    /// Returns true if buffer is empty, false otherwise.
    pub fn is_empty(&self) -> bool { self.size == 0 }

    /// Returns available data slot to be written.
    pub fn available_space(&self) -> usize { BUFFER_CAPACITY - self.size }

    /// Returns available data to be read.
    pub fn available_data(&self) -> usize { self.size }

    /// Adds an item to the buffer.
    pub fn push(&mut self, item: ItemType) {
        self.data[self.end] = item;
        self.end = Buffer::advance(self.end);
        if self.size < BUFFER_CAPACITY {
            self.size += 1;
        }
    }

    /// Remove an item at the front of the buffer.
    ///
    /// Returns None if buffer is empty, otherwise the result.
    #[must_use]
    pub fn pop(&mut self) -> Option<ItemType> {
        if self.is_empty() {
            return None;
        }
        let result = self.data[self.begin];
        self.begin = Buffer::advance(self.begin);
        self.size -= 1;
        Some(result)
    }

    /// Increments the begin or end marker and wrap around if necessary.
    fn advance(position: usize) -> usize { (position + 1) % BUFFER_CAPACITY }
}
