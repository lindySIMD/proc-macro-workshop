use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{
    parse::Parse, parse_macro_input, spanned::Spanned, visit_mut::VisitMut, Error, Item, ItemEnum,
    ItemFn, Pat, Path,
};

#[proc_macro_attribute]
pub fn sorted(_args: TokenStream, input: TokenStream) -> TokenStream {
    // match handle_sorted(input) {
    //     Ok(tokens) => tokens.into(),
    //     Err(e) => e.into_compile_error().into(),
    // }
    let sorted_input = parse_macro_input!(input as SortedInput);
    sorted_input.build()
}

#[proc_macro_attribute]
pub fn check(_args: TokenStream, input: TokenStream) -> TokenStream {
    let check_input = parse_macro_input!(input as CheckInput);
    check_input.build()
}

struct CheckInput {
    syntax_tree: ItemFn,
}

impl Parse for CheckInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let item: Item = input.parse()?;
        let Item::Fn(syntax_tree) = item else {
            return Err(Error::new(input.span(), "expected fn expression"));
        };
        Ok(Self { syntax_tree })
    }
}

struct ArmPath {
    path: Path,
}

impl From<Path> for ArmPath {
    fn from(path: Path) -> Self {
        Self { path }
    }
}

impl ArmPath {
    // fn new_from_ident(ident: &'a Ident) -> Self {
    //     Self { path: ident.into() }
    // }

    fn get_ident(&self) -> &Ident {
        &self.path.segments.last().unwrap().ident
    }

    fn span(&self) -> Span {
        self.path.span()
    }

    fn display_string(&self) -> String {
        let mut out = String::new();
        for (i, segment) in self.path.segments.iter().enumerate() {
            if i > 0 {
                out.push_str("::");
            }
            out.push_str(&segment.ident.to_string());
        }
        out
    }
}

impl VisitMut for Checker {
    fn visit_expr_match_mut(&mut self, item: &mut syn::ExprMatch) {
        let Some(sorted_attr_index) = item.attrs.iter().enumerate().find_map(|(index, attr)| {
            if attr.path().is_ident("sorted") {
                return Some(index);
            }
            None
        }) else {
            return;
        };
        item.attrs.remove(sorted_attr_index);
        let mut seen_arm_paths: Vec<ArmPath> = vec![];
        for (i, arm) in item.arms.iter().enumerate() {
            let this_path = match &arm.pat {
                Pat::Ident(ident) => ident.ident.clone().into(),
                Pat::Path(pat) => pat.path.clone(),
                Pat::TupleStruct(pat) => pat.path.clone(),
                Pat::Struct(pat) => pat.path.clone(),
                Pat::Wild(_) => {
                    if i == item.arms.len() - 1 {
                        return;
                    } else {
                        panic!("Bad underscore position");
                    }
                }
                _ => {
                    let error = Error::new(arm.pat.span(), "unsupported by #[sorted]");
                    self.error = Some(error);
                    return;
                }
            };
            let this_path: ArmPath = this_path.into();
            if let Some(last_path) = seen_arm_paths.last() {
                if this_path.get_ident() < last_path.get_ident() {
                    let before = seen_arm_paths
                        .iter()
                        .find(|seen| seen.get_ident() > this_path.get_ident())
                        .unwrap();
                    let error = Error::new(
                        this_path.span(),
                        &format!(
                            "{} should sort before {}",
                            this_path.display_string(),
                            before.display_string()
                        ),
                    );
                    self.error = Some(error);
                    return;
                }
            }
            seen_arm_paths.push(this_path);
        }
    }
}

#[derive(Default)]
struct Checker {
    error: Option<Error>,
}

impl CheckInput {
    fn build(self) -> TokenStream {
        let Self { mut syntax_tree } = self;
        let mut checker = Checker::default();
        checker.visit_item_fn_mut(&mut syntax_tree);
        let err_quote = if let Some(err) = checker.error {
            err.into_compile_error()
        } else {
            quote!()
        };
        quote!(
            #syntax_tree
            #err_quote
        )
        .into()
    }
}

struct SortedInput {
    item: ItemEnum,
}

impl Parse for SortedInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let item: Item = input.parse()?;
        let Item::Enum(item) = item else {
            return Err(Error::new(input.span(), "expected enum or match expression"));
        };
        Ok(Self { item })
    }
}

impl SortedInput {
    fn build(self) -> TokenStream {
        // Vis
        let item = self.item;
        let ItemEnum { variants, .. } = &item;
        let mut seen_variants: Vec<&syn::Variant> = vec![];
        let mut error = quote!();
        for variant in variants {
            if let Some(&last) = seen_variants.last() {
                if variant.ident < last.ident {
                    let before = seen_variants
                        .iter()
                        .find(|seen| seen.ident > variant.ident)
                        .unwrap();
                    error = Error::new(
                        variant.ident.span(),
                        &format!("{} should sort before {}", variant.ident, before.ident),
                    )
                    .into_compile_error();
                    break;
                }
            }
            seen_variants.push(variant);
        }

        quote!(
            #item
            #error
        )
        .into()
    }
}

// fn handle_sorted(input: TokenStream) -> Result<TokenStream2, Error> {
//     let sorted_input = parse_macro_input!(input as SortedInput);
//     let item = sorted_input.item;
//     Ok(quote!(#item))
// }
