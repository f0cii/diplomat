use super::formatter::CFormatter;
use super::header::Header;
use crate::ErrorStore;
use askama::Template;
use diplomat_core::hir::TypeContext;
use diplomat_core::hir::{
    self, CallbackInstantiationFunctionality, OpaqueOwner, ReturnableStructDef, StructPathLike,
    SymbolId, TraitIdGetter, TyPosition, Type, TypeDef, TypeId,
};
use std::borrow::Cow;
use std::fmt::Write;

#[derive(Template)]
#[template(path = "mojo/enum.mojo.jinja", escape = "none")]
struct EnumTemplate<'a> {
    ty: &'a hir::EnumDef,
    fmt: &'a CFormatter<'a>,
    ty_name: &'a str,
}

#[derive(Template)]
#[template(path = "mojo/struct.mojo.jinja", escape = "none")]
struct StructTemplate<'a> {
    ty_name: Cow<'a, str>,
    fields: Vec<(Cow<'a, str>, Cow<'a, str>)>,
}

#[derive(Template)]
#[template(path = "mojo/trait.mojo.jinja", escape = "none")]
struct TraitTemplate<'a> {
    trt_name: Cow<'a, str>,
    method_sigs: Vec<String>,
}

#[derive(Template)]
#[template(path = "mojo/opaque.mojo.jinja", escape = "none")]
struct OpaqueTemplate<'a> {
    ty_name: Cow<'a, str>,
}

#[derive(Template)]
#[template(path = "mojo/impl.mojo.jinja", escape = "none")]
struct ImplTemplate<'a> {
    methods: Vec<MethodTemplate<'a>>,
    cb_structs_and_defs: Vec<CallbackAndStructDef>,
    ty_name: Cow<'a, str>,
    dtor_name: Option<&'a str>,
    dtor_method_name: String,
}

struct MethodTemplate<'a> {
    return_ty: Cow<'a, str>,
    params: String,
    param_names: String,
    abi_name: &'a str,
    method_name: String,
}

#[derive(Clone)]
struct CallbackAndStructDef {
    name: String,
    params_types: String,
    return_type: String,
}

/// The context used for generating a particular type
pub struct TyGenContext<'cx, 'tcx> {
    pub tcx: &'tcx TypeContext,
    pub formatter: &'cx CFormatter<'tcx>,
    pub errors: &'cx ErrorStore<'tcx, String>,
    pub id: SymbolId,
    pub decl_header_path: &'cx String,
    pub impl_header_path: &'cx String,
}

impl<'cx, 'tcx> TyGenContext<'cx, 'tcx> {
    pub fn gen_enum_def(&self, def: &'tcx hir::EnumDef) -> Header {
        let mut decl_header = Header::new(self.decl_header_path.clone());
        let ty_name = self.formatter.fmt_type_name(self.id.try_into().unwrap());
        EnumTemplate {
            ty: def,
            fmt: self.formatter,
            ty_name: &ty_name,
        }
        .render_into(&mut decl_header)
        .unwrap();

        decl_header
    }

    pub fn gen_opaque_def(&self, _def: &'tcx hir::OpaqueDef) -> Header {
        let mut decl_header = Header::new(self.decl_header_path.clone());
        let ty_name = self.formatter.fmt_type_name(self.id.try_into().unwrap());
        OpaqueTemplate { ty_name }
            .render_into(&mut decl_header)
            .unwrap();

        decl_header
    }

    pub fn gen_struct_def<P: TyPosition>(&self, def: &'tcx hir::StructDef<P>) -> Header {
        let mut decl_header = Header::new(self.decl_header_path.clone());
        let ty_name = self.formatter.fmt_type_name(self.id.try_into().unwrap());
        let mut fields = vec![];
        let mut cb_structs_and_defs = vec![];
        for field in def.fields.iter() {
            fields.push(self.gen_ty_decl(
                &field.ty,
                field.name.as_str(),
                &mut decl_header,
                None,
                &mut cb_structs_and_defs, // for now this gets ignored, there are no callbacks in struct fields
            ));
        }

        StructTemplate { ty_name, fields }
            .render_into(&mut decl_header)
            .unwrap();

        decl_header
    }

    pub fn gen_trait_def(&self, def: &'tcx hir::TraitDef) -> Header {
        let mut decl_header = Header::new(self.decl_header_path.clone());
        let trt_name = self.formatter.fmt_trait_name(self.id.try_into().unwrap());
        let mut method_sigs = vec![];
        for m in &def.methods {
            let mut param_types: Vec<Cow<'tcx, str>> = m
                .params
                .iter()
                .map(|param| self.gen_ty_name(&param.ty, &mut decl_header))
                .collect();
            param_types.insert(0, "void*".into());
            let ret_type = if m.output.is_some() {
                self.gen_ty_name(&m.output.clone().unwrap(), &mut decl_header)
            } else {
                "void".into()
            };
            method_sigs.push(format!(
                "{} (*run_{}_callback)({});",
                ret_type,
                m.name.as_ref().unwrap().as_str(),
                param_types.join(", ")
            ));
        }

        TraitTemplate {
            trt_name,
            method_sigs,
        }
        .render_into(&mut decl_header)
        .unwrap();

        decl_header
    }

    pub fn gen_impl(&self, ty: hir::TypeDef<'tcx>) -> Header {
        let mut impl_header = Header::new(self.impl_header_path.clone());
        let mut methods = vec![];
        let mut cb_structs_and_defs = vec![];
        for method in ty.methods() {
            if method.attrs.disable {
                // Skip method if disabled
                continue;
            }
            let _guard = self.errors.set_context_method(
                self.tcx.fmt_symbol_name_diagnostics(self.id),
                method.name.as_str().into(),
            );
            let (method_chunk, callback_defs) = self.gen_method(method, &mut impl_header);
            methods.push(method_chunk);
            cb_structs_and_defs.extend_from_slice(&callback_defs);
        }

        let ty_name = self.formatter.fmt_type_name(self.id.try_into().unwrap());

        let dtor_name = if let TypeDef::Opaque(opaque) = ty {
            Some(opaque.dtor_abi_name.as_str())
        } else {
            None
        };
        let dtor_method_name = dtor_name.unwrap_or("").to_lowercase();
        ImplTemplate {
            ty_name,
            methods,
            cb_structs_and_defs,
            dtor_name,
            dtor_method_name,
        }
        .render_into(&mut impl_header)
        .unwrap();

        impl_header.decl_include = Some(self.decl_header_path.clone());

        // In some cases like generating decls for `self` parameters,
        // a header will get its own includes. Instead of
        // trying to avoid pushing them, it's cleaner to just pull them out
        // once done
        impl_header.includes.remove(self.impl_header_path);
        impl_header.includes.remove(self.decl_header_path);

        impl_header
    }

    fn gen_method(
        &self,
        method: &'tcx hir::Method,
        header: &mut Header,
    ) -> (MethodTemplate<'tcx>, Vec<CallbackAndStructDef>) {
        use diplomat_core::hir::{ReturnType, SuccessType};
        let abi_name = method.abi_name.as_str();
        // Right now these are the same, but we may eventually support renaming
        // and should be sure to use method_name when naming the result type
        let method_name = abi_name;
        let mut param_decls = Vec::new();
        let mut cb_structs_and_defs = vec![];
        if let Some(ref self_ty) = method.param_self {
            let self_ty = self_ty.ty.clone().into();
            param_decls.push(self.gen_ty_decl(
                &self_ty,
                "self",
                header,
                Some(abi_name.into()),
                &mut cb_structs_and_defs,
            ))
        }

        for param in &method.params {
            param_decls.push(self.gen_ty_decl(
                &param.ty,
                param.name.as_str(),
                header,
                Some(abi_name.into()),
                &mut cb_structs_and_defs,
            ));
        }

        let return_ty: Cow<str> = match method.output {
            // todo:
            ReturnType::Infallible(SuccessType::Unit) => "None".into(), // todo:void
            ReturnType::Infallible(SuccessType::Write) => {
                param_decls.push((
                    format!("UnsafePointer[{}]", self.formatter.fmt_write_name()).into(),
                    "write".into(),
                ));
                "None".into()
            }
            ReturnType::Infallible(SuccessType::OutType(ref o)) => self.gen_ty_name(o, header),
            ReturnType::Fallible(ref ok, _) | ReturnType::Nullable(ref ok) => {
                // Result<T, ()> and Option<T> are the same on the ABI
                let err = if let ReturnType::Fallible(_, Some(ref e)) = method.output {
                    Some(e)
                } else {
                    None
                };
                let ok_ty = match ok {
                    SuccessType::Write => {
                        param_decls.push((
                            format!("{}*", self.formatter.fmt_write_name()).into(),
                            "write".into(),
                        ));
                        None
                    }
                    SuccessType::Unit => None,
                    SuccessType::OutType(o) => Some(o),
                    _ => unreachable!("unknown AST/HIR variant"),
                };
                self.gen_result_ty(method_name, ok_ty, err, header).into()
            }
            _ => unreachable!("unknown AST/HIR variant"),
        };

        let mut params = String::new();
        let mut first = true;
        for (decl_ty, decl_name) in &param_decls {
            let comma = if first {
                first = false;
                ""
            } else {
                ", "
            };
            write!(&mut params, "{comma}{decl_name}: {decl_ty}").unwrap();
        }

        if params.is_empty() {
            params.push_str("");
        }

        let mut param_names = String::new();
        let mut first = true;
        for (decl_ty, decl_name) in &param_decls {
            let comma = if first {
                first = false;
                ""
            } else {
                ", "
            };
            write!(&mut param_names, "{comma}{decl_name}").unwrap();
        }

        if param_names.is_empty() {
            param_names.push_str("");
        }

        let method_name = abi_name.to_lowercase();

        (
            MethodTemplate {
                abi_name,
                method_name,
                return_ty,
                params,
                param_names,
            },
            cb_structs_and_defs,
        )
    }

    fn gen_result_ty(
        &self,
        fn_name: &str,
        ok_ty: Option<&hir::OutType>,
        err_ty: Option<&hir::OutType>,
        header: &mut Header,
    ) -> String {
        let ok_ty = ok_ty.filter(|t| {
            let Type::Struct(s) = t else {
                return true;
            };
            match s.resolve(self.tcx) {
                ReturnableStructDef::Struct(s) => !s.fields.is_empty(),
                ReturnableStructDef::OutStruct(s) => !s.fields.is_empty(),
                _ => unreachable!("unknown AST/HIR variant"),
            }
        });

        let err_ty = err_ty.filter(|t| {
            let Type::Struct(s) = t else {
                return true;
            };
            match s.resolve(self.tcx) {
                ReturnableStructDef::Struct(s) => !s.fields.is_empty(),
                ReturnableStructDef::OutStruct(s) => !s.fields.is_empty(),
                _ => unreachable!("unknown AST/HIR variant"),
            }
        });

        let ok_name = if let Some(ok) = ok_ty {
            let ok_name = self.gen_ty_name(ok, header);
            format!("{ok_name}")
        } else {
            "".into()
        };

        let oi_name = match ok_name.as_str() {
            "c_int8_t" => "I8",
            "c_int8" => "I8",
            "c_uint8_t" => "U8",
            "c_uint8" => "U8",
            "c_int16_t" => "I16",
            "c_int16" => "I16",
            "c_uint16_t" => "U16",
            "c_uint16" => "U16",
            "c_int32_t" => "I32",
            "c_int32" => "I32",
            "c_uint32_t" => "U32",
            "c_uint32" => "U32",
            "c_int64_t" => "I64",
            "c_int64" => "I64",
            "c_uint64_t" => "U64",
            "c_uint64" => "U64",
            "intptr_t" => "Isize",
            "c_size_t" => "Usize",
            "c_size" => "Usize",
            "c_float" => "F32",
            "c_double" => "F64",
            "c_bool" => "Bool",
            "c_char32_t" => "Char",
            "c_char32" => "Char",
            "c_char" => "String",
            "c_char16_t" => "String16",
            "c_char16" => "String16",
            "DiplomatStringView" => "Strings",
            "DiplomatString16View" => "Strings16",
            _ => unreachable!("unknown type {ok_name}"),
        };
        format!("Option{oi_name}Result")
    }

    /// Generates a decl for a given type, returned as (type, name)
    fn gen_ty_decl<'a, P: TyPosition>(
        &self,
        ty: &Type<P>,
        ident: &'a str,
        header: &mut Header,
        method_abi_name: Option<String>,
        cb_structs_and_defs: &mut Vec<CallbackAndStructDef>,
    ) -> (Cow<'tcx, str>, Cow<'a, str>) {
        let param_name = self.formatter.fmt_param_name(ident);
        match ty {
            Type::Callback(some_cb) => {
                let cb_wrapper_type = "DiplomatCallback_".to_owned()
                    + method_abi_name.unwrap().as_str()
                    + "_"
                    + ident;
                let params = some_cb.get_inputs().unwrap();
                let output_type = Box::new(some_cb.get_output_type().unwrap().clone());
                // this call generates any imports needed for param + output type(s)
                cb_structs_and_defs.push(self.gen_cb_param_wrapper_struct(
                    &cb_wrapper_type,
                    params,
                    &output_type,
                    header,
                ));
                (
                    cb_wrapper_type.clone().into(),
                    format!("{}_cb_wrap", param_name).into(),
                )
            }
            Type::ImplTrait(t) => {
                let t_id = t.id();
                let trt_name = self.gen_ty_name(ty, header);
                if self.tcx.resolve_trait(t_id).attrs.disable {
                    self.errors
                        .push_error(format!("Found usage of disabled trait {trt_name}"))
                }
                (
                    format!("DiplomatTraitStruct_{}", trt_name).into(),
                    format!("{}_trait_wrap", param_name).into(),
                )
            }
            _ => {
                let ty = self.gen_ty_name(ty, header);
                (ty, param_name)
            }
        }
    }

    fn gen_cb_param_wrapper_struct(
        &self,
        cb_wrapper_type: &str,
        params: &[hir::CallbackParam],
        output_type: &Option<Type>,
        header: &mut Header,
    ) -> CallbackAndStructDef {
        let return_type = if output_type.is_some() {
            self.gen_ty_name(&(*output_type).clone().unwrap(), header)
                .into()
        } else {
            "void".into()
        };
        let params_types = params
            .iter()
            .map(|p| self.gen_ty_name(&p.ty, header).to_string())
            .collect::<Vec<String>>()
            .join(", ");

        CallbackAndStructDef {
            name: cb_wrapper_type.into(),
            params_types,
            return_type,
        }
    }

    // Generate the C code for referencing a particular type in the input position (i.e., not return types).
    // Handles adding imports and such as necessary
    pub fn gen_ty_name<P: TyPosition>(&self, ty: &Type<P>, header: &mut Header) -> Cow<'tcx, str> {
        let ty_name = match *ty {
            Type::Primitive(prim) => self.formatter.fmt_primitive_as_mojo(prim),
            Type::Opaque(ref op) => {
                let op_id: TypeId = op.tcx_id.into();
                let ty_name = self.formatter.fmt_type_name_maybe_namespaced(op_id.into());
                if self.tcx.resolve_type(op_id).attrs().disable {
                    self.errors
                        .push_error(format!("Found usage of disabled type {ty_name}"))
                }
                // unwrap_or(mut) since owned pointers need to not be const
                let mutability = op.owner.mutability().unwrap_or(hir::Mutability::Mutable);
                let ret = self.formatter.fmt_ptr(&ty_name, mutability);
                header
                    .includes
                    .insert(self.formatter.fmt_decl_header_path(op_id.into()));
                ret.into_owned().into()
            }
            Type::Struct(ref st) => {
                let st_id = st.id();
                let ty_name = self.formatter.fmt_type_name_maybe_namespaced(st_id.into());
                if self.tcx.resolve_type(st_id).attrs().disable {
                    self.errors
                        .push_error(format!("Found usage of disabled type {ty_name}"))
                }
                let ret = ty_name.clone();
                let header_path = self.formatter.fmt_decl_header_path(st_id.into());
                header.includes.insert(header_path);
                ret
            }
            Type::Enum(ref e) => {
                let id: TypeId = e.tcx_id.into();
                let ty_name = self.formatter.fmt_type_name_maybe_namespaced(id.into());
                if self.tcx.resolve_type(id).attrs().disable {
                    self.errors
                        .push_error(format!("Found usage of disabled type {ty_name}"))
                }
                let header_path = self.formatter.fmt_decl_header_path(id.into());
                header.includes.insert(header_path);
                ty_name
            }
            Type::Slice(ref s) => match s {
                hir::Slice::Primitive(borrow, prim) => {
                    self.formatter.fmt_primitive_slice_name(*borrow, *prim)
                }
                hir::Slice::Str(_, encoding) => self.formatter.fmt_str_view_name(*encoding),
                hir::Slice::Strs(encoding) => self.formatter.fmt_strs_view_name(*encoding),
                &_ => unreachable!("unknown AST/HIR variant"),
            },
            Type::DiplomatOption(ref s) => {
                let inner = self.gen_ty_name(s, header);
                self.formatter.fmt_optional_type_name(s, &inner).into()
            }
            Type::ImplTrait(ref t) => {
                let t_id = t.id();
                let trt_name = self.formatter.fmt_type_name_maybe_namespaced(t_id.into());
                if self.tcx.resolve_trait(t_id).attrs.disable {
                    self.errors
                        .push_error(format!("Found usage of disabled trait {trt_name}"))
                }
                let ret = trt_name.clone();
                let header_path = self.formatter.fmt_decl_header_path(t_id.into());
                header.includes.insert(header_path);
                ret
            }
            _ => unreachable!("{}", format!("unknown AST/HIR variant: {:?}", ty)),
        };

        ty_name
    }
}