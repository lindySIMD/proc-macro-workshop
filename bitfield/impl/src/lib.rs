use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{parse::Parse, parse_macro_input, Error, Field, Item, ItemStruct};

#[proc_macro_attribute]
pub fn bitfield(args: TokenStream, input: TokenStream) -> TokenStream {
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

    fn get_size_expr(&self) -> TokenStream2 {
        let mut size = quote!(0u8);
        for field in self.iter_fields() {
            let ident = &field.ty;
            size = quote!(
                #size + <#ident as Specifier>::BITS
            );
        }
        quote! {
            ((#size) / 8u8 ) as usize
        }
    }

    fn get_setters_and_getters(&self) -> TokenStream2 {
        let mut offset_inner = quote!(0usize);
        let mut setters_and_getters = quote!();
        for field in self.iter_fields() {
            let field_name = field.ident.as_ref().unwrap();
            let fty = &field.ty;
            let set_get_ty = quote!(<#fty as Specifier>::SetGetType);
            let bits = quote!(<#fty as Specifier>::BITS as usize);
            // let offset_fn_ident = format_ident!("offset_{}", field_name);
            let offset = quote!(
                    (#offset_inner)
            );
            let index_fn_ident = format_ident!("indexes_{}", field_name);
            let index_fn = quote!(
                const fn #index_fn_ident() -> (usize, usize, usize) {
                    let start_bit = #offset;
                    let finish_bit = #offset + #bits;
                    let start_byte = start_bit / 8;
                    let start_bit_offset = start_bit % 8;
                    let finish_byte = finish_bit / 8;
                    (start_byte, finish_byte, start_bit_offset)
                }
            );
            let set_fn_ident = format_ident!("set_{}", field_name);
            let get_fn_ident = format_ident!("get_{}", field_name);
            setters_and_getters = quote!(
                #setters_and_getters
                #index_fn
                pub fn #set_fn_ident(&mut self, #field_name: #set_get_ty) {
                    let (start_byte, finish_byte, mut bit_offset) = Self::#index_fn_ident();
                    let shift_amt = #set_get_ty::BITS as usize - #bits;
                    let shifted = #field_name << shift_amt;
                    let bytes = #field_name.to_le_bytes();
                    for (i, set_byte) in bytes.iter().enumerate() {
                        let old_byte = self.data[i];
                        let masked = old_byte & (u8::MAX << bit_offset);
                    }
                }
            );
        }
        setters_and_getters
    }

    fn get_struct_expr(&self) -> TokenStream2 {
        let vis = &self.item.vis;
        let ident = &self.item.ident;
        let size = self.get_size_expr();
        quote!(
            #vis struct #ident {
                data: [u8; #size]
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

#[proc_macro]
pub fn create_b_types(_input: TokenStream) -> TokenStream {
    let mut out = quote!();
    for i in 1..=64u8 {
        let ident = format_ident!("B{}", i);
        let set_get_type_raw = get_set_get_value(i);
        let set_get_type = format_ident!("u{}", set_get_type_raw);
        out = quote!(
            #out
            pub enum #ident {}
            impl Specifier for #ident {
                const BITS: u8 = #i;
                type SetGetType = #set_get_type;
            }
        );
    }
    out.into()
}

fn get_set_get_value(num: u8) -> u8 {
    let mut i = 8;
    while i < num {
        i *= 2;
    }
    i
}
