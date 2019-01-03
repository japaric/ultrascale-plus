use std::collections::HashMap;

use syn::{
    braced,
    parse::{self, Parse, ParseStream},
    spanned::Spanned,
    token::Brace,
    AttrStyle, Attribute, Expr, Ident, Item, PathArguments, Token, Type, TypeTuple,
};

pub struct Input {
    _const_token: Token![const],
    _ident: Ident,
    _colon_token: Token![:],
    _ty: TypeTuple,
    _eq_token: Token![=],
    _brace_token: Brace,
    pub items: Vec<Item>,
    _semi_token: Token![;],
}

impl Parse for Input {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        fn parse_items(input: ParseStream) -> parse::Result<Vec<Item>> {
            let mut items = vec![];

            while !input.is_empty() {
                items.push(input.parse()?);
            }

            Ok(items)
        }

        let content;
        Ok(Input {
            _const_token: input.parse()?,
            _ident: input.parse()?,
            _colon_token: input.parse()?,
            _ty: input.parse()?,
            _eq_token: input.parse()?,
            _brace_token: braced!(content in input),
            items: content.call(parse_items)?,
            _semi_token: input.parse()?,
        })
    }
}

pub type Statics = HashMap<Ident, Static>;

pub struct Static {
    pub attrs: Vec<Attribute>,
    pub mutability: Option<Token![mut]>,
    pub ty: Box<Type>,
}

pub struct Amp {
    /// Definitions (e.g. struct, enum) that may be used by the `shared` statics
    pub defs: Vec<Item>,
    pub items: Vec<Item>,
    pub shared: Statics,
}

impl Amp {
    pub fn parse(items_: Vec<Item>) -> parse::Result<Self> {
        let mut defs = vec![];
        let mut items = vec![];
        let mut shared = Statics::new();

        for item in items_ {
            match item {
                Item::Static(mut item) => {
                    if let Some(pos) = item.attrs.iter().position(|attr| eq(attr, "shared")) {
                        item.attrs.swap_remove(pos);

                        if shared.contains_key(&item.ident) {
                            return Err(parse::Error::new(
                                item.ident.span(),
                                "this static is listed twice",
                            ));
                        }

                        match &*item.expr {
                            Expr::Tuple(tuple) if tuple.elems.is_empty() => {}
                            _ => {
                                return Err(parse::Error::new(
                                    item.expr.span(),
                                    "`#[shared]` statics must be left uninitialized \
                                     (e.g. `static FOO: u32 = ()`)",
                                ));
                            }
                        }

                        shared.insert(
                            item.ident,
                            Static {
                                attrs: item.attrs,
                                mutability: item.mutability,
                                ty: item.ty,
                            },
                        );
                    } else {
                        items.push(Item::Static(item));
                    }
                }
                // TODO do other
                Item::Enum(item) => {
                    defs.push(Item::Enum(item));
                }
                _ => {
                    items.push(item);
                }
            }
        }

        Ok(Amp {
            defs,
            items,
            shared,
        })
    }
}

fn eq(attr: &Attribute, name: &str) -> bool {
    attr.style == AttrStyle::Outer && attr.path.segments.len() == 1 && {
        let pair = attr.path.segments.first().unwrap();
        let segment = pair.value();
        segment.arguments == PathArguments::None && segment.ident.to_string() == name
    }
}
