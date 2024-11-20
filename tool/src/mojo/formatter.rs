//! This module contains functions for formatting types

use diplomat_core::hir::{
    self, StringEncoding, SymbolId, TraitId, TyPosition, TypeContext, TypeId,
};
use std::borrow::Cow;

/// This type mediates all formatting
///
/// All identifiers from the HIR should go through here before being formatted
/// into the output: This makes it easy to handle reserved words or add rename support
///
/// If you find yourself needing an identifier formatted in a context not yet available here, please add a new method
///
/// This type may be used by other backends attempting to figure out the names
/// of C types and methods.
pub struct CFormatter<'tcx> {
    tcx: &'tcx TypeContext,
}

impl<'tcx> CFormatter<'tcx> {
    pub fn new(tcx: &'tcx TypeContext) -> Self {
        Self { tcx }
    }
    pub fn tcx(&self) -> &'tcx TypeContext {
        self.tcx
    }

    /// Resolve and format a named type for use in code (without the namespace)
    pub fn fmt_type_name(&self, id: TypeId) -> Cow<'tcx, str> {
        let resolved = self.tcx.resolve_type(id);
        let name: Cow<_> = resolved.name().as_str().into();
        let attrs = resolved.attrs();

        // Only apply renames in cpp mode, in pure C mode you'd want the
        name
    }

    pub fn fmt_trait_name(&self, id: TraitId) -> Cow<'tcx, str> {
        let resolved = self.tcx.resolve_trait(id);
        let name: Cow<_> = resolved.name.as_str().into();
        let attrs = &resolved.attrs;

        name
    }

    /// Given a type found inside a DiplomatOption<T>, provide the name of the corresponding option type
    ///
    /// ty_name may or may not have namespacing done to it, you can use the result of `fmt_type_name`, `fmt_type_name_maybe_namespaced`,
    /// or something more complex here.
    pub fn fmt_optional_type_name<P: TyPosition>(
        &self,
        ty: &hir::Type<P>,
        ty_name: &str,
    ) -> String {
        match ty {
            hir::Type::Primitive(prim) => self.diplomat_namespace(format!("Option{}", self.fmt_primitive_name_for_derived_type(*prim)).into()).into(),
            hir::Type::Struct(..) | hir::Type::Enum(..) => format!("{ty_name}_option"),
            _ => unreachable!("Called fmt_optional_type_name with type {ty_name}, which is not allowed inside an Option")
        }
    }

    /// Resolve and format a named type for use in code (with a namespace, if needed by C++)
    pub fn fmt_type_name_maybe_namespaced(&self, id: SymbolId) -> Cow<'tcx, str> {
        let (name, attrs) = match id {
            SymbolId::TypeId(id) => {
                let resolved = self.tcx.resolve_type(id);
                let name: Cow<_> = resolved.name().as_str().into();
                let attrs = resolved.attrs();
                (name, attrs)
            }
            SymbolId::TraitId(id) => {
                let resolved = self.tcx.resolve_trait(id);
                let name: Cow<_> = resolved.name.as_str().into();
                let attrs = &resolved.attrs;
                (name, attrs)
            }
            _ => panic!("Unexpected symbol ID type"),
        };
        // Only apply renames in cpp mode, in pure C mode you'd want the
        // method names to match the type names.
        self.diplomat_namespace(name)
    }

    /// Resolve and format the name of a type for use in header names: decl version
    //
    /// Enums can't be forward-declared in C, but we do want enums to have methods,
    /// which may require additional #includes leading to potential cycles.
    /// To handle this, we make a separate header file called Foo_decl.h, that contains
    /// *just* the enum. It is included from Foo.h, and external users should not be importing
    /// it directly. (We can potentially add a #define guard that makes this actually private, if needed)
    pub fn fmt_decl_header_path(&self, id: SymbolId) -> String {
        let type_name = match id {
            SymbolId::TypeId(id) => self.fmt_type_name(id),
            SymbolId::TraitId(id) => self.fmt_trait_name(id),
            _ => panic!("Unexpected symbol ID type"),
        };
        let type_name = type_name.to_lowercase();
        format!("{type_name}_d.mojo")
    }
    /// Resolve and format the name of a type for use in header names: impl version
    pub fn fmt_impl_header_path(&self, id: SymbolId) -> String {
        let type_name = match id {
            SymbolId::TypeId(id) => self.fmt_type_name(id),
            SymbolId::TraitId(id) => self.fmt_trait_name(id),
            _ => panic!("Unexpected symbol ID type"),
        };
        let type_name = type_name.to_lowercase();
        format!("{type_name}.mojo")
    }

    /// Format an enum variant.
    pub fn fmt_enum_variant(
        &self,
        type_name: &str,
        variant: &'tcx hir::EnumVariant,
    ) -> Cow<'tcx, str> {
        let variant_name = variant.name.as_str();
        format!("{type_name}_{variant_name}").into()
    }
    /// Format a field name or parameter name
    // might need splitting in the future if we decide to support renames here
    pub fn fmt_param_name<'a>(&self, ident: &'a str) -> Cow<'a, str> {
        ident.into()
    }

    pub fn fmt_ptr<'a>(&self, ident: &'a str, mutability: hir::Mutability) -> Cow<'a, str> {
        // TODO: Where is the right place to put `const` here?
        if mutability.is_mutable() {
            format!("UnsafePointer[{ident}]").into()
        } else {
            format!("UnsafePointer[{ident}]").into()
        }
    }

    /// Get the primitive type as a C type
    pub fn fmt_primitive_as_mojo(&self, prim: hir::PrimitiveType) -> Cow<'static, str> {
        use diplomat_core::hir::{FloatType, IntSizeType, IntType, PrimitiveType};
        let s = match prim {
            PrimitiveType::Bool => "c_bool",

            PrimitiveType::Char => "c_char32",
            PrimitiveType::Int(IntType::I8) => "c_int8",
            PrimitiveType::Int(IntType::U8) | PrimitiveType::Byte => "c_uint8",
            PrimitiveType::Int(IntType::I16) => "c_int16",
            PrimitiveType::Int(IntType::U16) => "c_uint16",
            PrimitiveType::Int(IntType::I32) => "c_int32",
            PrimitiveType::Int(IntType::U32) => "c_uint32",
            PrimitiveType::Int(IntType::I64) => "c_int64",
            PrimitiveType::Int(IntType::U64) => "c_uint64",
            PrimitiveType::Int128(_) => panic!("i128 not supported in C"),
            PrimitiveType::IntSize(IntSizeType::Isize) => "c_intptr",
            PrimitiveType::IntSize(IntSizeType::Usize) => "c_size_t",
            PrimitiveType::Float(FloatType::F32) => "c_float",
            PrimitiveType::Float(FloatType::F64) => "c_double",
        };
        s.into()
    }

    /// Get the primitive name as used in a "derived" type (like slices and options)
    pub fn fmt_primitive_name_for_derived_type(&self, prim: hir::PrimitiveType) -> &'static str {
        use diplomat_core::hir::{FloatType, IntSizeType, IntType, PrimitiveType};
        match prim {
            PrimitiveType::Bool => "Bool",
            PrimitiveType::Char => "Char",
            PrimitiveType::Int(IntType::I8) => "I8",
            PrimitiveType::Int(IntType::U8) | PrimitiveType::Byte => "U8",
            PrimitiveType::Int(IntType::I16) => "I16",
            PrimitiveType::Int(IntType::U16) => "U16",
            PrimitiveType::Int(IntType::I32) => "I32",
            PrimitiveType::Int(IntType::U32) => "U32",
            PrimitiveType::Int(IntType::I64) => "I64",
            PrimitiveType::Int(IntType::U64) => "U64",
            PrimitiveType::Int128(_) => panic!("i128 not supported in C"),
            PrimitiveType::IntSize(IntSizeType::Isize) => "Isize",
            PrimitiveType::IntSize(IntSizeType::Usize) => "Usize",
            PrimitiveType::Float(FloatType::F32) => "F32",
            PrimitiveType::Float(FloatType::F64) => "F64",
        }
    }
    /// Get the primitive type as a C type
    pub fn fmt_primitive_slice_name(
        &self,
        borrow: Option<hir::Borrow>,
        prim: hir::PrimitiveType,
    ) -> Cow<'tcx, str> {
        let prim = self.fmt_primitive_name_for_derived_type(prim);
        let mtb = match borrow {
            Some(borrow) if borrow.mutability.is_immutable() => "",
            _ => "Mut",
        };
        self.diplomat_namespace(format!("Diplomat{prim}View{mtb}").into())
    }

    pub(crate) fn fmt_write_name(&self) -> Cow<'tcx, str> {
        self.diplomat_namespace("DiplomatWrite".into())
    }

    pub(crate) fn fmt_str_view_name(&self, encoding: StringEncoding) -> Cow<'tcx, str> {
        self.diplomat_namespace(
            match encoding {
                hir::StringEncoding::UnvalidatedUtf16 => "DiplomatString16View",
                _ => "DiplomatStringView",
            }
            .into(),
        )
    }

    pub(crate) fn fmt_strs_view_name(&self, encoding: StringEncoding) -> Cow<'tcx, str> {
        self.diplomat_namespace(
            match encoding {
                hir::StringEncoding::UnvalidatedUtf16 => "DiplomatStrings16View",
                _ => "DiplomatStringsView",
            }
            .into(),
        )
    }

    fn diplomat_namespace(&self, ty: Cow<'tcx, str>) -> Cow<'tcx, str> {
        ty
    }
}