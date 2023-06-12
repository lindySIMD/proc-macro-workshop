use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse_macro_input, punctuated::Punctuated, spanned::Spanned, token::Comma, Data, DeriveInput,
    Error, Expr, ExprAssign, ExprLit, Field, Fields, GenericArgument, Ident, Lit, Path,
    PathArguments, Result, Type, TypePath,
};
// use syn::pa

// pub trait Builder {}

enum BuilderStructField<'a> {
    Incremental {
        ident: &'a Ident,
        ty: &'a Type,
        each: Ident,
    },
    Required {
        ident: &'a Ident,
        ty: &'a Type,
    },
    Optional {
        ident: &'a Ident,
        ty: &'a Type,
        inner_ty: &'a Punctuated<GenericArgument, Comma>,
        // each: Option<String>,
    },
}

fn extract_each_attr(field: &Field) -> Option<Result<Ident>> {
    let attrs = &field.attrs;
    let attr = attrs.get(0)?;
    if !attr.path().is_ident("builder") {
        return Some(Err(Error::new(
            attr.meta.span(),
            "expected `builder(each = \"...\")`",
        )));
    }
    let metalist = attr.meta.require_list().unwrap();
    let args: ExprAssign = metalist.parse_args().unwrap();
    let args_str = args.to_token_stream().to_string();
    if !args_str.contains("each = \"") {
        return Some(Err(Error::new(
            attr.meta.span(),
            "expected `builder(each = \"...\")`",
        )));
    }
    let Expr::Lit(ExprLit { lit: Lit::Str(each), .. }) = args.right.as_ref() else {
        return Some(Err(Error::new(
            attr.meta.span(),
            "expected `builder(each = \"...\")`",
        )));
    };
    let ident = Ident::new(&each.value(), each.span());
    Some(Ok(ident))
}

fn new_builder_struct_field(field: &Field) -> Result<BuilderStructField> {
    let ident = field.ident.as_ref().unwrap();
    let ty = &field.ty;
    if let Some(each) = extract_each_attr(field) {
        let each = each?;
        return Ok(BuilderStructField::Incremental { ident, ty, each });
    }
    if let Type::Path(TypePath {
        path: Path { segments, .. },
        ..
    }) = ty
    {
        if let Some(first_segment) = segments.first() {
            let ident_str = first_segment.ident.to_string();
            if &ident_str == "Option" {
                let PathArguments::AngleBracketed(generic_args) = &first_segment.arguments else {
                    panic!("Malformed option generic argument");
                };
                let inner_ty = &generic_args.args;
                return Ok(BuilderStructField::Optional {
                    ident,
                    ty,
                    inner_ty,
                });
            }
        }
    }
    Ok(BuilderStructField::Required { ident, ty })
}

struct BuilderStructInfo<'a> {
    name: &'a Ident,
    builder_name: Ident,
    fields: Vec<BuilderStructField<'a>>,
}

impl<'a> BuilderStructField<'a> {
    fn init_repr(&self) -> TokenStream2 {
        let ident = match *self {
            Self::Incremental { ident, .. } => {
                return quote!(#ident: std::vec::Vec::new(),);
            }
            Self::Required { ident, .. } => ident,
            Self::Optional { ident, .. } => ident,
        };
        quote!(#ident: None,)
    }

    fn builder_field(&self) -> TokenStream2 {
        match *self {
            Self::Incremental { ident, ty, .. } => {
                quote!(#ident: #ty,)
            }
            Self::Required { ident, ty, .. } => {
                quote!(#ident: std::option::Option<#ty>,)
            }
            Self::Optional { ident, ty, .. } => {
                quote!(#ident: #ty,)
            }
        }
    }

    fn checks(&self) -> TokenStream2 {
        match *self {
            Self::Incremental { .. } => {
                quote!()
            }
            Self::Required { ident, .. } => {
                let error_string = format!(
                    "Builder Error: tried to build without setting {}.",
                    ident.to_string()
                );
                quote!(
                    if self.#ident.is_none() {
                        return Err(String::from(#error_string).into());
                    }
                )
            }
            Self::Optional { .. } => {
                quote!()
            }
        }
    }

    fn take(&self) -> TokenStream2 {
        match *self {
            Self::Incremental { ident, .. } => {
                quote!(#ident: self.#ident.clone(),)
            }
            Self::Required { ident, .. } => {
                quote!(#ident: self.#ident.take().unwrap(),)
            }
            Self::Optional { ident, .. } => {
                quote!(#ident: self.#ident.take(),)
            }
        }
    }

    fn methods(&self) -> TokenStream2 {
        match *self {
            Self::Incremental {
                ident,
                ty,
                ref each,
            } => {
                let each_ident = each;
                let Type::Path(TypePath { path: Path { segments, .. }, .. }) = ty else {
                        panic!("Field {} marked with each attribute was not a Vec<T>", ident.to_string());
                    };
                let Some(PathArguments::AngleBracketed(generic_args)) = segments.first().map(|seg| &seg.arguments) else {
                        panic!("Field {} marked with each attribute was not a Vec<T>", ident.to_string());
                    };
                let generic_args = &generic_args.args;
                let each_method = quote!(
                    fn #each_ident(&mut self, #each_ident: #generic_args) -> &mut Self {
                        self.#ident.push(#each_ident);
                        self
                    }
                );
                if each_ident.to_string() == ident.to_string() {
                    return each_method;
                }
                quote!(
                    fn #ident(&mut self, #ident: #ty) -> &mut Self {
                        self.#ident = #ident;
                        self
                    }
                    #each_method
                )
            }
            Self::Required { ident, ty } => {
                quote!(
                    fn #ident(&mut self, #ident: #ty) -> &mut Self {
                        self.#ident = Some(#ident);
                        self
                    }
                )
            }
            Self::Optional {
                ident, inner_ty, ..
            } => {
                quote!(
                    fn #ident(&mut self, #ident: #inner_ty) -> &mut Self {
                        self.#ident = Some(#ident);
                        self
                    }
                )
            }
        }
    }
}

// let tokens = quote!(
//     use std::error::Error;
//     pub struct #builder_struct {
//         #( #field_idents: Option<#field_types>,)*
//         #( #optional_idents: #optional_types,)*
//     }

//     impl #builder_struct {
//         #(
//             fn #field_idents(&mut self, #field_idents: #field_types) -> &mut Self {
//                 self.#field_idents = Some(#field_idents);
//                 self
//             }

//         )*

//         // TODO: ADD CODE FOR OPTIONAL STUFF
//         #(
//             fn #optional_idents(&mut self, #optional_idents: #optional_generic_types) -> &mut Self {
//                 self.#optional_idents = Some(#optional_idents);
//                 self
//             }
//         )*

//         pub fn build(&mut self) -> Result<#struct_name, Box<dyn Error>> {
//             #(
//                 if self.#field_idents.is_none() {
//                     return Err(String::from(#error_messages).into());
//                 }
//             )*
//             Ok(#struct_name {
//                 #(
//                     #field_idents: self.#field_idents.take().unwrap(),
//                 )*
//                 #(
//                     #optional_idents: self.#optional_idents.take()
//                 ),*
//             })
//         }
//     }

//     impl #struct_name {
//         pub fn builder() -> #builder_struct {
//             #builder_struct {
//                 #( #field_idents: None,)*
//                 #( #optional_idents: None,)*
//             }
//         }
//     }
// );

impl<'a> BuilderStructInfo<'a> {
    fn from_input(input: &'a DeriveInput) -> Result<Self> {
        let name = &input.ident;
        let builder_name = format_ident!("{}Builder", name);
        let Data::Struct(data) = &input.data else {
            panic!("Builder can only be derived for structs");
        };
        let Fields::Named(fields) = &data.fields else {
            panic!("Builder can only be derived for structs with named fields");
        };
        let fields = fields
            .named
            .iter()
            .map(new_builder_struct_field)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            name,
            builder_name,
            fields,
        })
    }

    fn builder_struct(&self) -> TokenStream2 {
        let builder_name = &self.builder_name;
        let builder_fields = self.fields.iter().map(BuilderStructField::builder_field);
        let builder_struct = quote!(
            pub struct #builder_name {
                #(#builder_fields)*
            }
        );
        let build_method = self.build_method();
        let builder_methods = self.fields.iter().map(BuilderStructField::methods);
        let builder_impl = quote!(
            impl #builder_name {
                #(#builder_methods)*
                #build_method
            }
        );
        quote!(
            #builder_struct
            #builder_impl
        )
    }

    fn build_method(&self) -> TokenStream2 {
        let struct_name = self.name;
        let checks = self.fields.iter().map(BuilderStructField::checks);
        let takes = self.fields.iter().map(BuilderStructField::take);
        quote!(
            pub fn build(&mut self) -> std::result::Result<#struct_name, std::boxed::Box<dyn Error>> {
                #(#checks)*
                Ok(#struct_name {
                    #(#takes)*
                })
            }
        )
    }

    fn fields_init(&self) -> TokenStream2 {
        let init_reprs = self.fields.iter().map(BuilderStructField::init_repr);
        quote!(
            #( #init_reprs)*
        )
    }

    fn builder_method(&self) -> TokenStream2 {
        let struct_name = self.name;
        let builder_name = &self.builder_name;
        let fields_init = self.fields_init();
        quote!(
            impl #struct_name {
                pub fn builder() -> #builder_name {
                    #builder_name {
                        #fields_init
                    }
                }
            }
        )
    }

    fn generate_tokens(&self) -> TokenStream2 {
        let builder_struct = self.builder_struct();
        let builder_method = self.builder_method();
        quote!(
            use std::error::Error;
            #builder_struct
            #builder_method
        )
    }
}

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    // let _ = input;
    // let input = parse_macro_input
    // let input = parse_macro_input
    let input = parse_macro_input!(input as DeriveInput);
    // eprintln!("input: {:#?}", input);
    // let struct_name = input.ident;
    // let builder_struct = format_ident!("{}Builder", struct_name);
    // let fields = if let Data::Struct(this) = input.data {
    //     if let Fields::Named(these_fields) = this.fields {
    //         these_fields
    //     } else {
    //         panic!("Builder can only be derived for structs with named fields");
    //     }
    // } else {
    //     panic!("Builder can only be derived for structs");
    // };
    // // let mut option_fields = vec![];
    // let mut optional_idents = vec![];
    // let mut optional_types = vec![];
    // let mut optional_generic_types = vec![];
    // let (field_idents, field_types): (Vec<_>, Vec<_>) = fields
    //     .named
    //     .iter()
    //     .filter_map(|field| {
    //         if let Type::Path(TypePath {
    //             path: Path { segments, .. },
    //             ..
    //         }) = &field.ty
    //         {
    //             if let Some(first_segment) = segments.first() {
    //                 let ident_str = first_segment.ident.to_string();
    //                 if &ident_str == "Option" {
    //                     optional_idents.push(field.ident.as_ref().unwrap());
    //                     optional_types.push(&field.ty);
    //                     let PathArguments::AngleBracketed(generic_args) = &first_segment.arguments else {
    //                         panic!("Malformed option generic argument");
    //                     };
    //                     let generic_args = &generic_args.args;
    //                     optional_generic_types.push(generic_args);
    //                     return None;
    //                 }
    //             }
    //         }
    //         Some((field.ident.as_ref().unwrap(), &field.ty))
    //     })
    //     .unzip();
    // let error_messages = field_idents
    //     .iter()
    //     .map(|ident| {
    //         format!(
    //             "{} Error: tried to build without setting {}.",
    //             builder_struct.to_string(),
    //             ident.to_string()
    //         )
    //     })
    //     .collect::<Vec<_>>();
    // // optional_generic_types.iter().for_each(|args| {
    // //     let tokens = quote!(#args);
    // //     eprintln!("ARGS: {}", tokens);
    // // });
    // let tokens = quote!(
    //     use std::error::Error;
    //     pub struct #builder_struct {
    //         #( #field_idents: Option<#field_types>,)*
    //         #( #optional_idents: #optional_types,)*
    //     }

    //     impl #builder_struct {
    //         #(
    //             fn #field_idents(&mut self, #field_idents: #field_types) -> &mut Self {
    //                 self.#field_idents = Some(#field_idents);
    //                 self
    //             }

    //         )*

    //         // TODO: ADD CODE FOR OPTIONAL STUFF
    //         #(
    //             fn #optional_idents(&mut self, #optional_idents: #optional_generic_types) -> &mut Self {
    //                 self.#optional_idents = Some(#optional_idents);
    //                 self
    //             }
    //         )*

    //         pub fn build(&mut self) -> Result<#struct_name, Box<dyn Error>> {
    //             #(
    //                 if self.#field_idents.is_none() {
    //                     return Err(String::from(#error_messages).into());
    //                 }
    //             )*
    //             Ok(#struct_name {
    //                 #(
    //                     #field_idents: self.#field_idents.take().unwrap(),
    //                 )*
    //                 #(
    //                     #optional_idents: self.#optional_idents.take()
    //                 ),*
    //             })
    //         }
    //     }

    //     impl #struct_name {
    //         pub fn builder() -> #builder_struct {
    //             #builder_struct {
    //                 #( #field_idents: None,)*
    //                 #( #optional_idents: None,)*
    //             }
    //         }
    //     }
    // );
    // eprintln!("OUTPUT: {}", tokens);
    let builderbuilder = match BuilderStructInfo::from_input(&input) {
        Ok(b) => b,
        Err(e) => return e.into_compile_error().into(),
    };
    let tokens = builderbuilder.generate_tokens();
    tokens.into()
}
