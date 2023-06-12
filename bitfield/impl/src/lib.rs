use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{parse::Parse, parse_macro_input, Error, Field, Item, ItemStruct};

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

    fn get_size_expr(&self) -> TokenStream2 {
        let mut size = quote!(0usize);
        for field in self.iter_fields() {
            let ident = &field.ty;
            size = quote!(
                #size + <#ident as Specifier>::BITS
            );
        }
        quote! {
            ((#size) / 8)
        }
    }

    fn get_setters_and_getters(&self) -> TokenStream2 {
        let mut offset_inner = quote!(0usize);
        let mut setters_and_getters = quote!();
        for field in self.iter_fields() {
            let field_name = field.ident.as_ref().unwrap();
            let fty = &field.ty;
            let set_get_ty = quote!(<#fty as Specifier>::SetGetType);
            let set_fn_ident = format_ident!("set_{}", field_name);
            let get_fn_ident = format_ident!("get_{}", field_name);
            setters_and_getters = quote!(
                #setters_and_getters
                pub fn #set_fn_ident(&mut self, #field_name: #set_get_ty) {
                    <Self as bitfield::BitField>::set_field::<#fty, {#offset_inner}>(self, #field_name)
                }
                pub fn #get_fn_ident(&self) -> #set_get_ty {
                    <Self as bitfield::BitField>::get_field::<#fty, {#offset_inner}>(self)
                }
            );
            let bits = quote!(<#fty as Specifier>::BITS);
            offset_inner = quote!( #offset_inner + #bits );
        }
        setters_and_getters
    }

    fn get_struct_expr(&self) -> TokenStream2 {
        let vis = &self.item.vis;
        let ident = &self.item.ident;
        let size = self.get_size_expr();
        let setters_and_getters = self.get_setters_and_getters();
        quote!(
            #vis struct #ident {
                data: [u8; #size]
            }

            impl BitField for #ident {
                const SIZE: usize = #size;
                fn get_byte(&self, index: usize) -> u8 {
                    self.data[index]
                }

                fn set_byte(&mut self, index: usize, byte: u8) {
                    self.data[index] = byte;
                }
            }

            impl #ident {
                pub fn new() -> #ident {
                    Self {
                        data: [0u8; #size]
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

fn get_set_get_value(num: usize) -> usize {
    let mut i = 8;
    while i < num {
        i *= 2;
    }
    i
}
