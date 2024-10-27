use askama::Template;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;
use std::fmt::Write;

static BASE_INCLUDES: &str = r#"
from memory import UnsafePointer
from sys.ffi import DLHandle
from .diplomat_runtime import *
"#;

/// This abstraction allows us to build up headers piece by piece without needing
/// to precalculate things like the list of dependent headers or forward declarations
#[derive(Default)]
pub struct Header {
    /// The path name used for the header file (for example Foo.mojo)
    pub path: String,
    /// A list of includes
    ///
    /// Example:
    /// ```mojo
    /// from .Foo import *
    /// from .diplomat_runtime import *
    /// ```
    pub includes: BTreeSet<String>,
    /// The decl file corresponding to this impl file. Empty if this is not an impl file.
    pub decl_include: Option<String>,
    /// The actual meat of the header: usually will contain a type definition and methods
    ///
    /// Example:
    /// ```mojo
    /// struct Foo:
    ///   field1: c_uint8
    ///   field2: c_bool
    ///
    /// make_foo(field1: c_uint8, field2: c_bool) -> Foo:
    /// ```
    pub body: String,
    /// What string to use for indentation.
    pub indent_str: &'static str,
}

impl Header {
    pub fn new(path: String) -> Self {
        Header {
            path,
            includes: BTreeSet::new(),
            decl_include: None,
            body: String::new(),
            indent_str: "  ",
        }
    }
}

impl fmt::Write for Header {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.body.write_str(s)
    }
    fn write_char(&mut self, c: char) -> fmt::Result {
        self.body.write_char(c)
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        self.body.write_fmt(args)
    }
}

#[derive(Template)]
#[template(path = "mojo/base.mojo.jinja", escape = "none")]
struct HeaderTemplate<'a> {
    header_guard: Cow<'a, str>,
    decl_include: Option<&'a String>,
    includes: &'a BTreeSet<String>,
    body: Cow<'a, str>,
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let header_guard = &self.path;
        let header_guard = header_guard.replace(".d.h", "_D_H");
        let header_guard = header_guard.replace(".h", "_H");
        let body: Cow<str> = if self.body.is_empty() {
            "// No Content\n\n".into()
        } else {
            self.body.replace('\t', self.indent_str).into()
        };

        HeaderTemplate {
            header_guard: header_guard.into(),
            includes: &self.includes,
            decl_include: self.decl_include.as_ref(),
            body,
        }
        .render_into(f)
        .unwrap();
        f.write_char('\n')
    }
}
