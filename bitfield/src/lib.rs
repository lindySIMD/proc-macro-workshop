use std::{
    fmt::{Binary, Debug},
    ops::{Index, IndexMut, Shl, Shr},
};

// Crates that have the "proc-macro" crate type are only allowed to export
// procedural macros. So we cannot have one crate that defines procedural macros
// alongside other types of public APIs like traits and structs.
//
// For this project we are going to need a #[bitfield] macro but also a trait
// and some structs. We solve this by defining the trait and structs in this
// crate, defining the attribute macro in a separate bitfield-impl crate, and
// then re-exporting the macro from this crate so that users only have one crate
// that they need to import.
//
// From the perspective of a user of this crate, they get all the necessary APIs
// (macro, trait, struct) through the one bitfield crate.
use bitfield_impl::create_b_types;
pub use bitfield_impl::{bitfield, BitfieldSpecifier};
// TODO other things

pub trait Specifier {
    const BITS: usize;
    type SetGetType: Default
        + AsBytes
        + Shr<usize, Output = Self::SetGetType>
        + Shl<usize, Output = Self::SetGetType>;
    const SHIFT_AMOUNT: usize = Self::SetGetType::BITS - Self::BITS;
    const OCCUPIED_BYTES: usize = Self::SetGetType::WIDTH - (Self::SHIFT_AMOUNT / 8);
}

pub trait BitfieldSpecifier {
    type Specifier: Specifier;
    type InOutType;
}

impl BitfieldSpecifier for bool {
    type Specifier = B1;
    type InOutType = bool;
}

impl BitfieldFrom<u8> for bool {
    fn from(val: u8) -> Self {
        val != 0
    }
}

pub trait BitfieldFrom<T> {
    fn from(val: T) -> Self;
}

impl<T> BitfieldFrom<T> for T
where
    T: AsBytes,
{
    fn from(val: T) -> Self {
        <Self as From<Self>>::from(val)
    }
}

impl<T> BitfieldSpecifier for T
where
    T: Specifier,
{
    type Specifier = T;
    type InOutType = T::SetGetType;
}

const fn tail_bits_mask(bit_offset: usize) -> u8 {
    u8::MAX >> bit_offset
}

const fn head_bits_mask(bit_offset: usize) -> u8 {
    shl_over(u8::MAX, bit_offset)
}

const fn shl_over(val: u8, shift: usize) -> u8 {
    match val.checked_shl(shift as u32) {
        Some(val) => val,
        None => 0,
    }
}

const fn bit_range_mask(start: usize, end: usize) -> u8 {
    (((u8::MAX << start) >> start) >> (8 - end)) << (8 - end)
}

fn start_end_bytes_and_bits(start_bit: usize, end_bit: usize) -> (usize, usize, usize, usize) {
    let start_byte = start_bit / 8;
    let end_byte = end_bit / 8;
    let start_bit_index = start_bit % 8;
    let end_bit_index = end_bit % 8;
    (start_byte, end_byte, start_bit_index, end_bit_index)
}

fn extract_bit_range(start_bit: usize, end_bit: usize, bytes: &[u8]) -> u64 {
    if end_bit - start_bit > 64 {
        panic!(
            "Tried to extract bit range from {} to {} > 64 bits",
            start_bit, end_bit
        );
    }
    let mut out = 0u64;
    let mut num_out_bits = 0usize;
    let (start_byte, end_byte, start_bit_index, end_bit_index) =
        start_end_bytes_and_bits(start_bit, end_bit);
    for index in start_byte..=end_byte {
        let byte_start_bit = if index == start_byte {
            start_bit_index
        } else {
            0
        };
        let byte_end_bit = if index == end_byte { end_bit_index } else { 8 };
        if byte_end_bit == 0 {
            break;
        }
        let mask = bit_range_mask(byte_start_bit, byte_end_bit);
        let masked_byte = bytes[index] & mask;
        let new_byte = masked_byte >> (8 - byte_end_bit);
        let num_new_bits = byte_end_bit - byte_start_bit;
        out = (out << num_new_bits) | (new_byte as u64);
        num_out_bits += num_new_bits;
    }
    assert_eq!(end_bit - start_bit, num_out_bits);
    out
}

// Takes input as u64 shifted so that bits 0..(end_bit-start_bit) are the information bits
fn set_bit_range(start_bit: usize, end_bit: usize, input: u64, bytes: &mut [u8]) {
    if end_bit - start_bit > 64 {
        panic!(
            "Tried to set bit range from {} to {} > 64 bits",
            start_bit, end_bit
        );
    }
    let mut num_set_bits = 0usize;
    let (start_byte, end_byte, start_bit_index, end_bit_index) =
        start_end_bytes_and_bits(start_bit, end_bit);
    for index in start_byte..=end_byte {
        let byte_start_bit = if index == start_byte {
            start_bit_index
        } else {
            0
        };
        let byte_end_bit = if index == end_byte { end_bit_index } else { 8 };
        if byte_end_bit == 0 {
            break;
        }
        let num_new_bits = byte_end_bit - byte_start_bit;
        // We want to write the upper bits first
        let new_bits = (input >> ((64 - num_new_bits) - num_set_bits)) as u8;
        // We only want num_new_bits bits here, so we put all the new bits in the low bits
        let bits_to_discard = 8 - num_new_bits;
        let new_bits_masked = (new_bits << bits_to_discard) >> bits_to_discard;
        let new_bits_in_place = new_bits_masked << (8 - byte_end_bit);
        let mask = bit_range_mask(byte_start_bit, byte_end_bit);
        let inv_mask = !mask;
        let masked_old_byte = bytes[index] & inv_mask;
        let new_byte = masked_old_byte | new_bits_in_place;
        bytes[index] = new_byte;
        num_set_bits += num_new_bits;
    }
}

pub trait BitField {
    const SIZE: usize;
    type SizeMod8: checks::TotalSizeIsMultipleOfEightBits;
    fn get_byte(&self, index: usize) -> u8;
    fn set_byte(&mut self, index: usize, byte: u8);
    fn get_data(&self) -> &[u8];
    fn get_data_mut(&mut self) -> &mut [u8];

    fn get_byte_masked(&self, index: usize, bit_offset: usize) -> u8 {
        let raw_byte = self.get_byte(index);
        raw_byte & head_bits_mask(8 - bit_offset)
    }

    fn get_offset_and_byte_range<T: Specifier, const OFFSET: usize>() -> (usize, usize, usize) {
        let bit_offset = OFFSET % 8;
        let start_byte = OFFSET / 8;
        let fin_byte = (OFFSET + T::BITS) / 8;
        (bit_offset, start_byte, fin_byte)
    }

    fn get_field<T: Specifier, const OFFSET: usize>(&self) -> T::SetGetType {
        let data = self.get_data();
        let extracted_bits = extract_bit_range(OFFSET, OFFSET + T::BITS, data);
        let extracted_bytes = extracted_bits.to_le_bytes();
        let num_out_bytes = T::OCCUPIED_BYTES;
        let mut out_bytes = <T::SetGetType as AsBytes>::Bytes::default();
        for i in 0..num_out_bytes {
            out_bytes[i] = extracted_bytes[i];
        }
        let out = T::SetGetType::from_bytes(out_bytes);
        // let shift_amount = T::SHIFT_AMOUNT;
        // out >> shift_amount
        out
    }

    fn _get_field<T: Specifier, const OFFSET: usize>(&self) -> T::SetGetType {
        eprintln!(
            "Getting {}, {:b}",
            std::any::type_name::<T>(),
            self.get_byte(0)
        );
        let mut start_byte_index = OFFSET / 8;
        let finish_byte_index = (OFFSET + T::BITS) / 8;
        let start_bit_offset = OFFSET % 8;
        let occupied_bytes = T::OCCUPIED_BYTES;
        let mut bytes = <T::SetGetType as AsBytes>::Bytes::default();
        let tail_bits_shl = 8 - start_bit_offset;
        let mut prev_byte = self.get_byte(start_byte_index);
        start_byte_index +=
            (start_bit_offset != 0 && finish_byte_index != start_byte_index) as usize;
        for index in 0..occupied_bytes {
            let this_byte = self.get_byte(start_byte_index + index);
            let out_byte = (this_byte >> start_bit_offset) | shl_over(prev_byte, tail_bits_shl);
            bytes[index] = out_byte;
            prev_byte = this_byte;
        }
        eprintln!("Got {}, {:?}", std::any::type_name::<T>(), bytes);
        let unshifted_out = T::SetGetType::from_bytes(bytes);
        // unshifted_out >> (T::SetGetType::BITS - T::BITS)
        unshifted_out >> T::SHIFT_AMOUNT
    }

    fn set_field<T: Specifier, const OFFSET: usize>(&mut self, val: T::SetGetType) {
        let in_as_u64 = val.into();
        let shift_amt = 64 - T::BITS;
        let shifted_in_u64 = in_as_u64 << shift_amt;
        let data_mut = self.get_data_mut();
        set_bit_range(OFFSET, OFFSET + T::BITS, shifted_in_u64, data_mut);
    }

    fn _set_field<T: Specifier, const OFFSET: usize>(&mut self, val: T::SetGetType) {
        eprintln!("Setting {} {:b}", std::any::type_name::<T>(), val);
        let mut start_byte_index = OFFSET / 8;
        let finish_byte_index = (OFFSET + T::BITS) / 8;
        let bit_offset = OFFSET % 8;
        let occupied_bytes = T::OCCUPIED_BYTES;
        let shifted_in = val << T::SHIFT_AMOUNT;
        let bytes = shifted_in.to_bytes();
        // Set first byte
        let first_byte = bytes[0];
        let prev_start_byte_masked = self.get_byte_masked(start_byte_index, bit_offset);
        let new_in_byte = prev_start_byte_masked | (first_byte >> bit_offset);
        self.set_byte(start_byte_index, new_in_byte);
        start_byte_index += (bit_offset != 0 && finish_byte_index != start_byte_index) as usize;
        let mut prev_byte = first_byte;
        let tail_bits_shl = 8 - bit_offset;
        for index in 1..occupied_bytes {
            let in_byte = bytes[index];
            let next_byte_val = shl_over(prev_byte, tail_bits_shl) | (in_byte >> bit_offset);
            self.set_byte(start_byte_index + index - 1, next_byte_val);
            prev_byte = in_byte;
        }
        // Now we just have the leftover from the last byte to set
        let tail_bit_index = OFFSET + T::BITS;
        let tail_bits = tail_bit_index % 8;
        // Mask everything after the tail bits
        let tail_mask = tail_bits_mask(tail_bits);
        let tail_byte_index = start_byte_index + (occupied_bytes - 1);
        let tail_byte = self.get_byte(tail_byte_index);
        let masked_tail_byte = tail_byte & tail_mask;
        let tail_bits_to_add = shl_over(prev_byte, 8 - tail_bits);
        let new_tail_byte = tail_bits_to_add | masked_tail_byte;
        self.set_byte(tail_byte_index, new_tail_byte);
    }
}

create_b_types!();

// This trick for enforcing compile time checks is HUGE:
//  https://stackoverflow.com/questions/32764797/how-to-enforce-that-a-type-implements-a-trait-at-compile-time
pub mod checks {
    use bitfield_impl::create_size_marker_types;
    pub trait TotalSizeIsMultipleOfEightBits {}
    #[derive(Default)]
    pub struct True;
    #[derive(Default)]
    pub struct False;
    pub trait DiscriminantCheck<const VALID: bool> {
        type Valid;
    }
    impl<T> DiscriminantCheck<true> for T {
        type Valid = True;
    }
    impl<T> DiscriminantCheck<false> for T {
        type Valid = False;
    }
    pub trait DiscriminantInRange {}
    impl DiscriminantInRange for True {}
    // impl DiscriminantInRange
    // pub trait TotalSize<const SIZE: usize> {}
    pub trait TotalSizeMod8<const SIZE: usize> {
        type Size;
    }
    create_size_marker_types!();
    impl TotalSizeIsMultipleOfEightBits for ZeroMod8 {}
}

// const fn size_mod_8<const SIZE: usize>(size: SIZE) ->

pub trait AsBytes: Binary + Into<u64> {
    const WIDTH: usize;
    const BITS: usize;
    type Bytes: Default + Debug + Index<usize, Output = u8> + IndexMut<usize, Output = u8>;
    fn to_bytes(&self) -> Self::Bytes;
    fn from_bytes(bytes: Self::Bytes) -> Self;
}

impl AsBytes for u8 {
    const WIDTH: usize = 1;
    const BITS: usize = Self::BITS as usize;
    type Bytes = [u8; Self::WIDTH];
    fn from_bytes(bytes: Self::Bytes) -> Self {
        Self::from_le_bytes(bytes)
    }

    fn to_bytes(&self) -> Self::Bytes {
        self.to_le_bytes()
    }
}

impl AsBytes for u16 {
    const WIDTH: usize = 2;
    const BITS: usize = Self::BITS as usize;
    type Bytes = [u8; Self::WIDTH];
    fn from_bytes(bytes: Self::Bytes) -> Self {
        Self::from_le_bytes(bytes)
    }

    fn to_bytes(&self) -> Self::Bytes {
        self.to_le_bytes()
    }
}

impl AsBytes for u32 {
    const WIDTH: usize = 4;
    const BITS: usize = Self::BITS as usize;
    type Bytes = [u8; Self::WIDTH];
    fn from_bytes(bytes: Self::Bytes) -> Self {
        Self::from_le_bytes(bytes)
    }

    fn to_bytes(&self) -> Self::Bytes {
        self.to_le_bytes()
    }
}

impl AsBytes for u64 {
    const WIDTH: usize = 8;
    const BITS: usize = Self::BITS as usize;
    type Bytes = [u8; Self::WIDTH];
    fn from_bytes(bytes: Self::Bytes) -> Self {
        Self::from_le_bytes(bytes)
    }

    fn to_bytes(&self) -> Self::Bytes {
        self.to_le_bytes()
    }
}
