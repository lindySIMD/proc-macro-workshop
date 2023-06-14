use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    parse::Parse, parse_macro_input, Data, DataEnum, DeriveInput, Error, Field, Item, ItemStruct,
    Visibility,
};

#[proc_macro_attribute]
pub fn bitfield(_args: TokenStream, input: TokenStream) -> TokenStream {
    let builder = parse_macro_input!(input as BitFieldBuilder);
    builder.build()
}

struct BitFieldBuilder {
    item: ItemStruct,
}

impl BitFieldBuilder {
    fn iter_fields(&self) -> impl Iterator<Item = &Field> {
        self.item.fields.iter()
    }

    fn get_bits_size_expr(&self) -> TokenStream2 {
        let mut size = quote!(0usize);
        for field in self.iter_fields() {
            let ident = &field.ty;
            size = quote!(
                #size + <<#ident as bitfield::BitfieldSpecifier>::Specifier as bitfield::Specifier>::BITS
            );
        }
        quote!((#size))
    }

    fn get_bytes_size_expr(&self) -> TokenStream2 {
        let mut size = quote!(0usize);
        for field in self.iter_fields() {
            let ident = &field.ty;
            size = quote!(
                #size + <<#ident as bitfield::BitfieldSpecifier>::Specifier as bitfield::Specifier>::BITS
            );
        }
        quote! {
            ((#size) / 8)
        }
    }

    fn get_setters_and_getters(&self) -> TokenStream2 {
        // let ident_strings: HashSet<String> =
        //     (1..=64).into_iter().map(|i| format!("B{}", i)).collect();
        let mut offset_inner = quote!(0usize);
        let mut setters_and_getters = quote!();
        for field in self.iter_fields() {
            let field_name = field.ident.as_ref().unwrap();
            let fty = &field.ty;
            // let set_get_ty = quote!(<#fty as Specifier>::SetGetType);
            let set_fn_ident = format_ident!("set_{}", field_name);
            let get_fn_ident = format_ident!("get_{}", field_name);
            let specifier_ty = quote!(<#fty as bitfield::BitfieldSpecifier>::Specifier);
            let set_get_ty = quote!(<#specifier_ty as bitfield::Specifier>::SetGetType);
            let in_out_ty = quote!(<#fty as bitfield::BitfieldSpecifier>::InOutType);
            setters_and_getters = quote!(
                #setters_and_getters
                pub fn #set_fn_ident(&mut self, #field_name: #in_out_ty) {
                    let into = #field_name as #set_get_ty;
                    <Self as bitfield::BitField>::set_field::<#specifier_ty, {#offset_inner}>(self, into)
                }
                pub fn #get_fn_ident(&self) -> #in_out_ty {
                    let raw = <Self as bitfield::BitField>::get_field::<#specifier_ty, {#offset_inner}>(self);
                    <#in_out_ty as bitfield::BitfieldFrom<#set_get_ty>>::from(raw)
                }
                // pub fn #set_fn_ident(&mut self, #field_name: #fty) {
                //     let into: #set_get_ty = #field_name.into();
                //     <Self as bitfield::BitField>::set_field::<#specifier_ty, {#offset_inner}>(self, #field_name)
                // }
                // pub fn #get_fn_ident(&self) -> #fty {
                //     let raw = <Self as bitfield::BitField>::get_field::<#specifier_ty, {#offset_inner}>(self);
                //     raw.into()
                // }
            );
            let bits = quote!(<#specifier_ty as bitfield::Specifier>::BITS);
            offset_inner = quote!( #offset_inner + #bits );
        }
        setters_and_getters
    }

    fn get_struct_expr(&self) -> TokenStream2 {
        let vis = &self.item.vis;
        let ident = &self.item.ident;
        let bytes_size = self.get_bytes_size_expr();
        let bits_size = self.get_bits_size_expr();
        let setters_and_getters = self.get_setters_and_getters();
        quote!(
            #vis struct #ident {
                data: [u8; #bytes_size]
            }

            impl BitField for #ident {
                const SIZE: usize = #bytes_size;
                type SizeMod8 = <bitfield::checks::SizeMarker as bitfield::checks::TotalSizeMod8<{#bits_size % 8}>>::Size;
                fn get_byte(&self, index: usize) -> u8 {
                    self.data[index]
                }

                fn set_byte(&mut self, index: usize, byte: u8) {
                    self.data[index] = byte;
                }

                fn get_data(&self) -> &[u8] {
                    &self.data
                }

                fn get_data_mut(&mut self) -> &mut [u8] {
                    &mut self.data
                }
            }

            impl #ident {
                pub fn new() -> #ident {
                    Self {
                        data: [0u8; #bytes_size]
                    }
                }
                #setters_and_getters
            }
        )
    }

    fn build(self) -> TokenStream {
        // let size_expr = self.get_size_expr();
        let struct_expr = self.get_struct_expr();
        quote!(
            #struct_expr
        )
        .into()
    }
}

impl Parse for BitFieldBuilder {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let item: Item = input.parse()?;
        let Item::Struct(item) = item else {
            return Err(Error::new(input.span(), "not a struct"));
        };
        Ok(Self { item })
    }
}

#[proc_macro_derive(BitfieldSpecifier)]
pub fn derive_macro_bitfield_specifier(input: TokenStream) -> TokenStream {
    let specifier_builder = parse_macro_input!(input as BitfieldSpecifierBuilder);
    specifier_builder.build()
}

struct BitfieldSpecifierBuilder {
    _vis: Visibility,
    ident: Ident,
    enum_data: DataEnum,
}

impl Parse for BitfieldSpecifierBuilder {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let input: DeriveInput = input.parse()?;
        let DeriveInput {
            vis, ident, data, ..
        } = input;
        let Data::Enum(enum_data) = data else {
            return Err(Error::new(Span::call_site(), "BitfieldSpecifier expected an enum"));
        };
        let num_variants = enum_data.variants.len();
        if !num_variants.is_power_of_two() {
            return Err(Error::new(
                Span::call_site(),
                "BitfieldSpecifier expected a number of variants which is a power of 2",
            ));
        }
        Ok(Self {
            _vis: vis,
            ident,
            enum_data,
        })
    }
}

impl BitfieldSpecifierBuilder {
    fn build(self) -> TokenStream {
        let Self {
            _vis: _,
            ident,
            enum_data,
        } = self;
        let num_vars = enum_data.variants.len();
        let num_bits = num_vars.checked_ilog2().unwrap() as usize;
        let b_ident = format_ident!("B{}", num_bits);
        let max_discriminant_val = 2usize.pow(num_bits as u32) - 1;
        // let max_discriminant_ident = format_ident!("{}", max_discriminant_val);
        // eprintln!("max ident: {}", quote!(#max_discriminant_val));
        let specifier = quote!(bitfield::#b_ident);
        let set_get_ty = quote!(<#specifier as bitfield::Specifier>::SetGetType);
        let mut from_arms = quote!();
        let mut const_var_defs = quote!();
        let discriminant_check_ident = format_ident!("{}DiscriminantCheck", ident);
        let mut discriminant_check_impls = quote!();
        for var in enum_data.variants.iter() {
            let var_ident = &var.ident;
            let var_struct_ident = format_ident!("{}VariantCheck", var_ident);
            let var_caps_string = var_ident.to_string().to_uppercase();
            let var_const_ident = format_ident!("BFCONST_{}", var_caps_string);
            from_arms = quote!(
                #from_arms
                Self::#var_const_ident => Self::#var_ident,
            );
            const_var_defs = quote!(
                #const_var_defs
                const #var_const_ident: #set_get_ty = Self::#var_ident as #set_get_ty;
            );
            discriminant_check_impls = quote!(
                #discriminant_check_impls
                struct #var_struct_ident;
                impl #discriminant_check_ident<<bool as bitfield::checks::DiscriminantCheck<{#ident::#var_const_ident <= #max_discriminant_val as #set_get_ty}>>::Valid> for #var_struct_ident {}
                // impl
            );
        }
        let ident_string = ident.to_string();
        let lower_ident_string = ident_string.to_lowercase();
        let discriminant_check_mod_ident =
            format_ident!("{}_discriminant_check", lower_ident_string);
        quote!(
            impl bitfield::BitfieldSpecifier for #ident {
                type Specifier = #specifier;
                type InOutType = #ident;
            }

            impl #ident {
                #const_var_defs
            }

            mod #discriminant_check_mod_ident {
                use super::*;
                trait #discriminant_check_ident<T: bitfield::checks::DiscriminantInRange> {}
                #discriminant_check_impls
            }


            impl bitfield::BitfieldFrom<#set_get_ty> for #ident {
                fn from(val: #set_get_ty) -> Self {
                    match val {
                        #from_arms
                        _ => panic!("Got invalid bits: 0b{:b} for {}", val, #ident_string),
                    }
                }
            }
        )
        .into()
    }
}

#[proc_macro]
pub fn create_b_types(_input: TokenStream) -> TokenStream {
    let mut out = quote!();
    for i in 1..=64usize {
        let ident = format_ident!("B{}", i);
        let set_get_type_raw = get_set_get_value(i);
        let set_get_type = format_ident!("u{}", set_get_type_raw);
        out = quote!(
            #out
            pub enum #ident {}
            impl Specifier for #ident {
                const BITS: usize = #i;
                type SetGetType = #set_get_type;
            }
        );
    }
    out.into()
}

#[proc_macro]
pub fn create_size_marker_types(_input: TokenStream) -> TokenStream {
    let num_names = [
        "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven",
    ];
    let mut out = quote!(
        pub struct SizeMarker;
    );
    for (i, num) in num_names.iter().enumerate() {
        let mod8ident = format_ident!("{}Mod8", num);
        out = quote!(
            #out
            pub struct #mod8ident;
            impl TotalSizeMod8<#i> for SizeMarker {
                type Size = #mod8ident;
            }
        );
    }
    out.into()
}

fn get_set_get_value(num: usize) -> usize {
    let mut i = 8;
    while i < num {
        i *= 2;
    }
    i
}
