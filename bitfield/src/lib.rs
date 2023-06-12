use std::{
    marker::PhantomData,
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
pub use bitfield_impl::bitfield;
use bitfield_impl::create_b_types;

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

const fn tail_bits_mask(bit_offset: usize) -> u8 {
    u8::MAX >> bit_offset
}

const fn head_bits_mask(bit_offset: usize) -> u8 {
    shl_over(u8::MAX, bit_offset)
}

// pub struct Me {}

// impl BitField for Me {
//     const SIZE: usize = 12;
//     fn get_byte(&self, index: usize) -> u8 {
//         0
//     }
//     fn set_byte(&mut self, index: usize, byte: u8) {
//         return;
//     }
// }

// impl Me {
//     fn mee(&self) -> u8 {
//         <Self as crate::BitField>::get_field::<B1, { 0 + <B2 as Specifier>::BITS }>(self)
//     }
// }
// pub struct C<T: checks::TotalSizeIsMultipleOfEightBits> {
//     t: std::marker::PhantomData<T>,
// }

// impl C<checks::SevenMod8> {}

// struct _M;

// trait _SizeOk: checks::TotalSizeIsMultipleOfEightBits {}

// impl _SizeOk for _M {}

// impl M {
// fn new() -> impl checks::TotalSizeIsMultipleOfEightBits {
//     M
// }
// }

const fn shl_over(val: u8, shift: usize) -> u8 {
    match val.checked_shl(shift as u32) {
        Some(val) => val,
        None => 0,
    }
}

pub trait BitField {
    const SIZE: usize;
    type SizeMod8: checks::TotalSizeIsMultipleOfEightBits;
    fn get_byte(&self, index: usize) -> u8;
    fn set_byte(&mut self, index: usize, byte: u8);

    fn get_byte_masked(&self, index: usize, bit_offset: usize) -> u8 {
        let raw_byte = self.get_byte(index);
        raw_byte & head_bits_mask(8 - bit_offset)
    }

    fn get_field<T: Specifier, const OFFSET: usize>(&self) -> T::SetGetType {
        let mut start_byte_index = OFFSET / 8;
        let start_bit_offset = OFFSET % 8;
        let occupied_bytes = T::OCCUPIED_BYTES;
        let mut bytes = <T::SetGetType as AsBytes>::Bytes::default();
        let tail_bits_shl = 8 - start_bit_offset;
        let mut prev_byte = self.get_byte(start_byte_index);
        start_byte_index += (start_bit_offset != 0) as usize;
        for index in 0..occupied_bytes {
            let this_byte = self.get_byte(start_byte_index + index);
            let out_byte = (this_byte >> start_bit_offset) | shl_over(prev_byte, tail_bits_shl);
            bytes[index] = out_byte;
            prev_byte = this_byte;
        }
        let unshifted_out = T::SetGetType::from_bytes(bytes);
        // unshifted_out >> (T::SetGetType::BITS - T::BITS)
        unshifted_out >> T::SHIFT_AMOUNT
    }

    fn set_field<T: Specifier, const OFFSET: usize>(&mut self, val: T::SetGetType) {
        let mut start_byte_index = OFFSET / 8;
        let bit_offset = OFFSET % 8;
        let occupied_bytes = T::OCCUPIED_BYTES;
        let shifted_in = val << T::SHIFT_AMOUNT;
        let bytes = shifted_in.to_bytes();
        // Set first byte
        let first_byte = bytes[0];
        let prev_start_byte_masked = self.get_byte_masked(start_byte_index, bit_offset);
        let new_in_byte = prev_start_byte_masked | (first_byte >> bit_offset);
        self.set_byte(start_byte_index, new_in_byte);
        start_byte_index += (bit_offset != 0) as usize;
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

pub mod checks {
    use bitfield_impl::create_size_marker_types;

    pub trait TotalSizeIsMultipleOfEightBits {}
    // pub trait TotalSize<const SIZE: usize> {}
    pub trait TotalSizeMod8<const SIZE: usize> {
        type Size;
    }
    create_size_marker_types!();
    impl TotalSizeIsMultipleOfEightBits for ZeroMod8 {}
}

// const fn size_mod_8<const SIZE: usize>(size: SIZE) ->

pub trait AsBytes {
    const WIDTH: usize;
    const BITS: usize;
    type Bytes: Default + Index<usize, Output = u8> + IndexMut<usize, Output = u8>;
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
