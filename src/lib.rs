use proc_macro::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::Data;
use syn::DataStruct;
use syn::DeriveInput;
use syn::Error;
use syn::Fields;
use syn::Ident;
use syn::Type;
use syn::parse_macro_input;

/// Numeric types and the atomic type that holds each one.
const NUMERICS: [(&str, &str); 14] = [
    ("u8", "AtomicU8"),
    ("u16", "AtomicU16"),
    ("u32", "AtomicU32"),
    ("u64", "AtomicU64"),
    ("u128", "AtomicU128"),
    ("usize", "AtomicUsize"),
    ("i8", "AtomicI8"),
    ("i16", "AtomicI16"),
    ("i32", "AtomicI32"),
    ("i64", "AtomicI64"),
    ("i128", "AtomicI128"),
    ("isize", "AtomicIsize"),
    ("f32", "AtomicF32"),
    ("f64", "AtomicF64"),
];

/// Counter that belongs to a group, with what the group methods call on it.
struct GroupMember {
    field: Ident,
    value_type: Ident,
    atomic: bool,
}

/// Generates `inc_`, `dec_`, `add_`, `sub_`, `set_`, `get_` methods for each numeric or atomic field, and group methods for each group.
#[proc_macro_derive(Counters, attributes(counters))]
pub fn derive_counters(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // read named fields
    if let Some(attribute) = input.attrs.iter().find(|attribute| attribute.path().is_ident("counters")) {
        return Error::new_spanned(attribute, "#[counters] goes on fields").to_compile_error().into();
    }
    let Data::Struct(DataStruct { fields: Fields::Named(fields), .. }) = &input.data else {
        return Error::new_spanned(&input.ident, "Counters needs a struct with named fields").to_compile_error().into();
    };

    // generate field methods
    let vis = &input.vis;
    let relaxed = quote!(::std::sync::atomic::Ordering::Relaxed);
    let mut methods = Vec::new();
    let mut groups: Vec<(Ident, Vec<GroupMember>)> = Vec::new();
    for field in &fields.named {
        let name = field.ident.as_ref().expect("named field");
        let mut field_groups: Vec<Ident> = Vec::new();
        for attribute in field.attrs.iter().filter(|attribute| attribute.path().is_ident("counters")) {
            let options = attribute.parse_nested_meta(|meta| {
                let option = meta.path.require_ident()?.to_string();
                match option.as_str() {
                    "group" => {
                        let mut group: Option<Ident> = None;
                        if meta.input.peek(syn::token::Paren) {
                            meta.parse_nested_meta(|argument| {
                                if argument.path.is_ident("name") {
                                    group = Some(argument.value()?.parse()?);
                                    return Ok(());
                                }
                                Err(argument.error("group arguments are name"))
                            })?;
                        }
                        let Some(group) = group else { return Err(meta.error("group needs name: group(name = <name>)")) };
                        if field_groups.contains(&group) {
                            return Err(Error::new_spanned(&group, format!("this field is already in group {group}")));
                        }
                        field_groups.push(group);
                        Ok(())
                    }
                    "log" | "extra" => Err(meta.error("log options moved to derive-selflog: #[derive(SelfLog)] with #[selflog(entry(...))] or #[selflog(extra(...))]")),
                    _ => Err(meta.error("field option must be group")),
                }
            });
            if let Err(error) = options {
                return error.to_compile_error().into();
            }
        }

        let type_name = match &field.ty {
            Type::Path(path) => path.path.segments.last().map(|segment| segment.ident.to_string()),
            _ => None,
        };
        let numeric = type_name.as_deref().and_then(|type_name| NUMERICS.iter().find(|(value_type, atomic_type)| type_name == *value_type || type_name == *atomic_type));
        let Some((value_type, atomic_type)) = numeric else {
            if let Some(group) = field_groups.first() {
                return Error::new_spanned(group, "group needs a numeric or atomic field").to_compile_error().into();
            }
            continue;
        };
        let value_type = format_ident!("{value_type}");
        let atomic = type_name.as_deref() == Some(*atomic_type);
        let float = value_type.to_string().starts_with('f');
        let one = if float { quote!(1.0) } else { quote!(1) };

        let [inc, dec, add, sub, set, get] = ["inc", "dec", "add", "sub", "set", "get"].map(|verb| format_ident!("{verb}_{name}"));
        let (maximum, minimum) = if float { ("", "") } else { (", stopping at the maximum", ", stopping at the minimum") };
        let [inc_doc, dec_doc, add_doc, sub_doc, set_doc, get_doc] = [
            format!("Adds one to `{name}`{maximum}."),
            format!("Subtracts one from `{name}`{minimum}."),
            format!("Adds the amount to `{name}`{maximum}."),
            format!("Subtracts the amount from `{name}`{minimum}."),
            format!("Replaces `{name}` with the value."),
            format!("Returns the current `{name}`."),
        ];
        let wide = value_type.to_string().ends_with("128");
        let (add_body, sub_body) = match (atomic, float) {
            (true, true) => (quote!(self.#name.fetch_add(amount, #relaxed);), quote!(self.#name.fetch_sub(amount, #relaxed);)),
            (true, false) if wide => (
                quote!(let _ = self.#name.fetch_update(#relaxed, #relaxed, |current| Some(current.saturating_add(amount)));),
                quote!(let _ = self.#name.fetch_update(#relaxed, #relaxed, |current| Some(current.saturating_sub(amount)));),
            ),
            (true, false) => {
                (quote!(self.#name.update(#relaxed, #relaxed, |current| current.saturating_add(amount));), quote!(self.#name.update(#relaxed, #relaxed, |current| current.saturating_sub(amount));))
            }
            (false, true) => (quote!(self.#name += amount;), quote!(self.#name -= amount;)),
            (false, false) => (quote!(self.#name = self.#name.saturating_add(amount);), quote!(self.#name = self.#name.saturating_sub(amount);)),
        };
        let (receiver, set_body, get_body) =
            if atomic { (quote!(&self), quote!(self.#name.store(value, #relaxed);), quote!(self.#name.load(#relaxed))) } else { (quote!(&mut self), quote!(self.#name = value;), quote!(self.#name)) };
        methods.push(quote! {
            #[doc = #inc_doc]
            #vis fn #inc(#receiver) { self.#add(#one); }
            #[doc = #dec_doc]
            #vis fn #dec(#receiver) { self.#sub(#one); }
            #[doc = #add_doc]
            #vis fn #add(#receiver, amount: #value_type) { #add_body }
            #[doc = #sub_doc]
            #vis fn #sub(#receiver, amount: #value_type) { #sub_body }
            #[doc = #set_doc]
            #vis fn #set(#receiver, value: #value_type) { #set_body }
            #[doc = #get_doc]
            #vis fn #get(&self) -> #value_type { #get_body }
        });
        for group in &field_groups {
            let member = GroupMember { field: name.clone(), value_type: value_type.clone(), atomic };
            match groups.iter_mut().find(|(existing, _)| existing == group) {
                Some((_, members)) => members.push(member),
                None => groups.push((group.clone(), vec![member])),
            }
        }
    }

    // generate group methods
    for (group, members) in &groups {
        if members.len() < 2 {
            return Error::new_spanned(group, format!("group {group} has one counter; a group needs two or more")).to_compile_error().into();
        }
        let value_type = &members[0].value_type;
        if members.iter().any(|member| member.value_type != *value_type) {
            return Error::new_spanned(group, format!("the counters in group {group} have different types")).to_compile_error().into();
        }
        let receiver = if members.iter().all(|member| member.atomic) { quote!(&self) } else { quote!(&mut self) };
        let [inc, dec, add, sub, set] = ["inc", "dec", "add", "sub", "set"].map(|verb| format_ident!("{verb}_group_{group}"));
        let [member_inc, member_dec, member_add, member_sub, member_set] =
            ["inc", "dec", "add", "sub", "set"].map(|verb| members.iter().map(|member| format_ident!("{verb}_{}", member.field)).collect::<Vec<_>>());
        let [inc_doc, dec_doc, add_doc, sub_doc, set_doc] = [
            format!("Adds one to every counter in group `{group}`."),
            format!("Subtracts one from every counter in group `{group}`."),
            format!("Adds the amount to every counter in group `{group}`."),
            format!("Subtracts the amount from every counter in group `{group}`."),
            format!("Replaces every counter in group `{group}` with the value."),
        ];
        methods.push(quote! {
            #[doc = #inc_doc]
            #vis fn #inc(#receiver) { #(self.#member_inc();)* }
            #[doc = #dec_doc]
            #vis fn #dec(#receiver) { #(self.#member_dec();)* }
            #[doc = #add_doc]
            #vis fn #add(#receiver, amount: #value_type) { #(self.#member_add(amount);)* }
            #[doc = #sub_doc]
            #vis fn #sub(#receiver, amount: #value_type) { #(self.#member_sub(amount);)* }
            #[doc = #set_doc]
            #vis fn #set(#receiver, value: #value_type) { #(self.#member_set(value);)* }
        });
    }

    // emit methods
    let ident = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    quote! {
        #[allow(dead_code)]
        impl #impl_generics #ident #type_generics #where_clause {
            #(#methods)*
        }
    }
    .into()
}
