// SPDX-License-Identifier: Apache-2.0

//! Helpful structure to deal with arrays with a size larger than  32 bytes

use crate::error::LargeArrayError;
use serde::{Deserialize, Serialize};
use std::{
    convert::{TryFrom, TryInto},
    ops::{Deref, DerefMut},
};

/// Large array structure to serialize and default arrays larger than 32 bytes.
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[repr(C)]
pub struct LargeArray<T, const N: usize>(#[serde(with = "serde_arrays")] [T; N])
where
    T: for<'a> Deserialize<'a> + Serialize;

impl<T, const N: usize> Default for LargeArray<T, N>
where
    T: std::marker::Copy + std::default::Default + for<'a> Deserialize<'a> + Serialize,
{
    fn default() -> Self {
        Self([T::default(); N])
    }
}

impl<T, const N: usize> TryFrom<Vec<T>> for LargeArray<T, N>
where
    T: std::marker::Copy + std::default::Default + for<'a> Deserialize<'a> + Serialize,
{
    type Error = LargeArrayError;

    fn try_from(vec: Vec<T>) -> Result<Self, Self::Error> {
        Ok(LargeArray(vec.try_into().map_err(|_| {
            LargeArrayError::VectorError("Vector is the wrong size".to_string())
        })?))
    }
}

impl<T, const N: usize> TryFrom<[T; N]> for LargeArray<T, N>
where
    T: std::marker::Copy + std::default::Default + for<'a> Deserialize<'a> + Serialize,
{
    type Error = LargeArrayError;

    fn try_from(array: [T; N]) -> Result<Self, Self::Error> {
        Ok(LargeArray(array))
    }
}

impl<T, const N: usize> TryFrom<&[T]> for LargeArray<T, N>
where
    T: std::marker::Copy + std::default::Default + for<'a> Deserialize<'a> + Serialize,
{
    type Error = LargeArrayError;

    fn try_from(slice: &[T]) -> Result<Self, Self::Error> {
        Ok(LargeArray(slice.try_into()?))
    }
}

impl<T, const N: usize> LargeArray<T, N>
where
    T: std::marker::Copy + std::default::Default + for<'a> Deserialize<'a> + Serialize,
{
    /// Get the large array as a regular array format
    pub fn as_array(&self) -> [T; N] {
        self.0
    }
}

impl<T, const N: usize> AsRef<[T]> for LargeArray<T, N>
where
    T: std::marker::Copy + std::default::Default + Serialize + for<'a> Deserialize<'a>,
{
    fn as_ref(&self) -> &[T] {
        self.0.as_slice()
    }
}

impl<T, const N: usize> AsMut<[T]> for LargeArray<T, N>
where
    T: std::marker::Copy + std::default::Default + Serialize + for<'a> Deserialize<'a>,
{
    fn as_mut(&mut self) -> &mut [T] {
        self.0.as_mut_slice()
    }
}

impl<T, const N: usize> Deref for LargeArray<T, N>
where
    T: std::marker::Copy + std::default::Default + Serialize + for<'a> Deserialize<'a>,
{
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, const N: usize> DerefMut for LargeArray<T, N>
where
    T: std::marker::Copy + std::default::Default + Serialize + for<'a> Deserialize<'a>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
