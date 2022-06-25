mod utils;

use anyhow::{Context, Error};
use once_cell::sync::Lazy;
use std::{borrow::Borrow, sync::Arc};
use wasm_bindgen::prelude::*;

use swc::{
    atoms::js_word,
    config::{
        ErrorFormat, ExperimentalOptions, JsMinifyOptions, Options, ParseOptions, SourceMapsConfig,
    },
    try_with_handler, Compiler, TransformOutput,
};

use swc_common::{
    errors::{ColorConfig, Handler},
    FileName, FilePathMapping, Globals, SourceFile, SourceMap, DUMMY_SP, GLOBALS,
};

use swc_ecmascript::{
    ast::{CallExpr, Callee, Expr, Lit, Number, UnaryExpr, UnaryOp},
    transforms::pass::noop,
    visit::{as_folder, Fold},
    visit::{VisitMut, VisitMutWith},
};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(js_name = "transformSync", typescript_type = "transformSync")]
#[allow(unused_variables)]
pub fn transform_sync(s: &str) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let c = compiler();

    let opts: Options = Options {
        ..Default::default()
    };

    let error_format = opts.experimental.error_format.unwrap_or_default();

    try_with_handler(
        c.cm.clone(),
        swc::HandlerOpts {
            ..Default::default()
        },
        |handler| {
            c.run(|| {
                let fm = c.cm.new_source_file(
                    if opts.filename.is_empty() {
                        FileName::Anon
                    } else {
                        FileName::Real(opts.filename.clone().into())
                    },
                    s.into(),
                );
                let out = transform(&c, fm, handler, &opts)?;

                JsValue::from_serde(&out).context("failed to serialize json")
            })
        },
    )
    .map_err(|e| convert_err(e, error_format))
}

fn transform(
    c: &Arc<Compiler>,
    fm: Arc<SourceFile>,
    handler: &Handler,
    opts: &Options,
) -> Result<TransformOutput, Error> {
    c.process_js_with_custom_pass(fm, None, handler, &opts, |_, _| noop(), |_, _| my_visitor())
        .context("failed to process input file")
}

fn compiler() -> Arc<Compiler> {
    static C: Lazy<Arc<Compiler>> = Lazy::new(|| {
        let cm = Arc::new(SourceMap::new(FilePathMapping::empty()));

        Arc::new(Compiler::new(cm))
    });

    C.clone()
}

fn convert_err(err: Error, error_format: ErrorFormat) -> JsValue {
    error_format.format(&err).into()
}

fn my_visitor() -> impl Fold {
    as_folder(MyVisitor)
}
struct MyVisitor;
impl VisitMut for MyVisitor {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        expr.visit_mut_children_with(self);
        match expr {
            Expr::Member(member_expr) => {
                let obj = member_expr.obj.borrow();
                match obj {
                    Expr::Ident(ident) => {
                        if ident.sym == *"console" {
                            match member_expr.prop.borrow() {
                                swc_ecmascript::ast::MemberProp::Ident(_) => {
                                    *expr = Expr::Unary(UnaryExpr {
                                        span: member_expr.span,
                                        op: UnaryOp::Void,
                                        arg: Box::new(Expr::Lit(Lit::Num(Number {
                                            span: member_expr.span,
                                            value: 0.0,
                                            raw: None,
                                        }))),
                                    })
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[test]
fn test_visitor() {
    let code = r#"
    if (foo) {
        console.log("Foo")
    } else {
        console.log("Bar")
    }"#;
    let c = compiler();

    let opts: Options = Options {
        ..Default::default()
    };

    let cm = Arc::<SourceMap>::default();

    let handler = Arc::new(Handler::with_tty_emitter(
        ColorConfig::Auto,
        true,
        false,
        Some(cm.clone()),
    ));

    let fm = c.cm.new_source_file(FileName::Anon, code.into());

    let result = transform(&c, fm, handler.borrow(), &opts).unwrap().code;

    assert_eq!(
        result,
        r#"if (foo) {
    void 0("Foo");
} else {
    void 0("Bar");
}
"#
    )
}
