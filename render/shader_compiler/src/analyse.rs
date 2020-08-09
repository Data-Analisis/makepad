use crate::ast::*;
use crate::builtin::{self, Builtin};
use crate::const_eval::ConstEvaluator;
use crate::const_gather::ConstGatherer;
use crate::dep_analyse::DepAnalyser;
use crate::env::{Env, Sym, VarKind};
use crate::error::Error;
use crate::ident::Ident;
use crate::shadergen::PropDef;
use crate::span::Span;
use crate::ty::Ty;
use crate::ty_check::TyChecker;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, BTreeSet};

pub fn analyse(shader: &ShaderAst, input_props: &[&PropDef], gather_all: bool) -> Result<(), Error> {
    let builtins = builtin::generate_builtins();
    let mut env = Env::new();
    env.push_scope();

    for prop in input_props {
        env.insert_sym(
            Span::default(),
            Ident::new(&prop.ident),
            Sym::TyVar {
                ty: prop.prop_id.shader_ty(),
            },
        )?;
    }
    for &ident in builtins.keys() {
        env.insert_sym(Span::default(), ident, Sym::Builtin)?;
    }
    env.push_scope();
    ShaderAnalyser {
        builtins,
        shader,
        env,
        gather_all,
    }
    .analyse_shader()
}

#[derive(Debug)]
struct ShaderAnalyser<'a> {
    builtins: HashMap<Ident, Builtin>,
    shader: &'a ShaderAst,
    env: Env,
    gather_all: bool,
}

impl<'a> ShaderAnalyser<'a> {
    fn ty_checker(&self) -> TyChecker {
        TyChecker {
            builtins: &self.builtins,
            shader: self.shader,
            env: &self.env,
        }
    }

    fn const_evaluator(&self) -> ConstEvaluator {
        ConstEvaluator {
            shader: self.shader,
        }
    }

    fn const_gatherer(&self) -> ConstGatherer {
        ConstGatherer {
            shader: self.shader,
            gather_all: self.gather_all,
        }
    }

    fn analyse_shader(&mut self) -> Result<(), Error> {
        *self.shader.const_table.borrow_mut() = Some(Vec::new());
        self.env.push_scope();
        for decl in &self.shader.decls {
            self.analyse_decl(decl)?;
        }
        for decl in &self.shader.decls {
            match decl {
                Decl::Fn(decl) => {
                    FnDefAnalyser {
                        builtins: &self.builtins,
                        shader: self.shader,
                        decl,
                        env: &mut self.env,
                        gather_all: self.gather_all,
                        is_inside_loop: false,
                    }
                    .analyse_fn_def()?;
                }
                _ => {}
            }
        }
        self.env.pop_scope();
        for decl in &self.shader.decls {
            match decl {
                Decl::Geometry(decl) => {
                    decl.is_used_in_fragment_shader.set(Some(false));
                }
                Decl::Instance(decl) => {
                    decl.is_used_in_fragment_shader.set(Some(false));
                }
                Decl::Fn(decl) => {
                    decl.is_used_in_vertex_shader.set(Some(false));
                    decl.is_used_in_fragment_shader.set(Some(false));
                }
                _ => {}
            }
        }
        self.analyse_call_tree(
            ShaderKind::Vertex,
            &mut Vec::new(),
            self.shader.find_fn_decl(Ident::new("vertex")).unwrap(),
        )?;
        self.analyse_call_tree(
            ShaderKind::Fragment,
            &mut Vec::new(),
            self.shader.find_fn_decl(Ident::new("pixel")).unwrap(),
        )?;
        let mut visited = HashSet::new();
        let vertex_decl = self.shader.find_fn_decl(Ident::new("vertex")).unwrap();
        self.propagate_deps(&mut visited, vertex_decl)?;
        let fragment_decl = self.shader.find_fn_decl(Ident::new("pixel")).unwrap();
        self.propagate_deps(&mut visited, fragment_decl)?;
        for &geometry_dep in fragment_decl.geometry_deps.borrow().as_ref().unwrap() {
            self.shader
                .find_geometry_decl(geometry_dep)
                .unwrap()
                .is_used_in_fragment_shader
                .set(Some(true));
        }
        for &instance_dep in fragment_decl.instance_deps.borrow().as_ref().unwrap() {
            self.shader
                .find_instance_decl(instance_dep)
                .unwrap()
                .is_used_in_fragment_shader
                .set(Some(true));
        }
        Ok(())
    }

    fn analyse_decl(&mut self, decl: &Decl) -> Result<(), Error> {
        match decl {
            Decl::Geometry(decl) => self.analyse_geometry_decl(decl),
            Decl::Const(decl) => self.analyse_const_decl(decl),
            Decl::Fn(decl) => self.analyse_fn_decl(decl),
            Decl::Instance(decl) => self.analyse_instance_decl(decl),
            Decl::Struct(decl) => self.analyse_struct_decl(decl),
            Decl::Texture(decl) => self.analyse_texture_decl(decl),
            Decl::Uniform(decl) => self.analyse_uniform_decl(decl),
            Decl::Varying(decl) => self.analyse_varying_decl(decl),
        }
    }

    fn analyse_geometry_decl(&mut self, decl: &GeometryDecl) -> Result<(), Error> {
        let ty = self.ty_checker().ty_check_ty_expr(&decl.ty_expr)?;
        match ty {
            Ty::Float | Ty::Vec2 | Ty::Vec3 | Ty::Vec4 => {}
            _ => {
                return Err(Error {
                    span: decl.span,
                    message: String::from(
                        "attribute must be either a floating-point scalar or vector",
                    ),
                })
            }
        }
        self.env.insert_sym(
            decl.span,
            decl.ident,
            Sym::Var {
                is_mut: false,
                ty,
                kind: VarKind::Geometry,
            },
        )
    }

    fn analyse_const_decl(&mut self, decl: &ConstDecl) -> Result<(), Error> {
        let expected_ty = self.ty_checker().ty_check_ty_expr(&decl.ty_expr)?;
        let actual_ty = self.ty_checker().ty_check_expr_with_expected_ty(
            decl.span,
            &decl.expr,
            &expected_ty,
        )?;
        self.const_evaluator().const_eval_expr(&decl.expr)?;
        self.env.insert_sym(
            decl.span,
            decl.ident,
            Sym::Var {
                is_mut: false,
                ty: actual_ty,
                kind: VarKind::Const,
            },
        )
    }

    fn analyse_fn_decl(&mut self, decl: &FnDecl) -> Result<(), Error> {
        for param in &decl.params {
            self.ty_checker().ty_check_ty_expr(&param.ty_expr)?;
        }
        let return_ty = decl
            .return_ty_expr
            .as_ref()
            .map(|return_ty_expr| self.ty_checker().ty_check_ty_expr(return_ty_expr))
            .transpose()?
            .unwrap_or(Ty::Void);
        if decl.ident == Ident::new("vertex") {
            match return_ty {
                Ty::Vec4 => {}
                _ => {
                    return Err(Error {
                        span: decl.span,
                        message: String::from(
                            "function `vertex` must return a value of type `vec4`",
                        ),
                    })
                }
            }
        } else if decl.ident == Ident::new("pixel") {
            match return_ty {
                Ty::Vec4 => {}
                _ => {
                    return Err(Error {
                        span: decl.span,
                        message: String::from(
                            "function `fragment` must return a value of type `vec4`",
                        ),
                    })
                }
            }
        } else {
            match return_ty {
                Ty::Array { .. } => {
                    return Err(Error {
                        span: decl.span,
                        message: String::from("functions can't return arrays"),
                    })
                }
                _ => {}
            }
        }
        *decl.return_ty.borrow_mut() = Some(return_ty);
        self.env.insert_sym(decl.span, decl.ident, Sym::Fn).ok();
        Ok(())
    }

    fn analyse_instance_decl(&mut self, decl: &InstanceDecl) -> Result<(), Error> {
        let ty = self.ty_checker().ty_check_ty_expr(&decl.ty_expr)?;
        match ty {
            Ty::Float | Ty::Vec2 | Ty::Vec3 | Ty::Vec4 => {}
            _ => {
                return Err(Error {
                    span: decl.span,
                    message: String::from(
                        "attribute must be either a floating-point scalar or vector",
                    ),
                })
            }
        }
        self.env.insert_sym(
            decl.span,
            decl.ident,
            Sym::Var {
                is_mut: false,
                ty,
                kind: VarKind::Instance,
            },
        )
    }

    fn analyse_struct_decl(&mut self, decl: &StructDecl) -> Result<(), Error> {
        for field in &decl.fields {
            self.ty_checker().ty_check_ty_expr(&field.ty_expr)?;
        }
        self.env.insert_sym(
            decl.span,
            decl.ident,
            Sym::TyVar {
                ty: Ty::Struct { ident: decl.ident },
            },
        )
    }

    fn analyse_texture_decl(&mut self, decl: &TextureDecl) -> Result<(), Error> {
        let ty = self.ty_checker().ty_check_ty_expr(&decl.ty_expr)?;
        match ty {
            Ty::Texture2D => {}
            _ => {
                return Err(Error {
                    span: decl.span,
                    message: String::from("texture must be a texture2D"),
                })
            }
        }
        self.env.insert_sym(
            decl.span,
            decl.ident,
            Sym::Var {
                is_mut: false,
                ty,
                kind: VarKind::Texture,
            },
        )
    }

    fn analyse_uniform_decl(&mut self, decl: &UniformDecl) -> Result<(), Error> {
        let ty = self.ty_checker().ty_check_ty_expr(&decl.ty_expr)?;
        self.env.insert_sym(
            decl.span,
            decl.ident,
            Sym::Var {
                is_mut: false,
                ty,
                kind: VarKind::Uniform,
            },
        )
    }

    fn analyse_varying_decl(&mut self, decl: &VaryingDecl) -> Result<(), Error> {
        let ty = self.ty_checker().ty_check_ty_expr(&decl.ty_expr)?;
        match ty {
            Ty::Float | Ty::Vec2 | Ty::Vec3 | Ty::Vec4 => {}
            _ => {
                return Err(Error {
                    span: decl.span,
                    message: String::from(
                        "varying must be either a floating-point scalar or vector",
                    ),
                })
            }
        }
        self.env.insert_sym(
            decl.span,
            decl.ident,
            Sym::Var {
                is_mut: true,
                ty,
                kind: VarKind::Varying,
            },
        )
    }

    fn analyse_call_tree(
        &mut self,
        kind: ShaderKind,
        call_stack: &mut Vec<Ident>,
        decl: &FnDecl,
    ) -> Result<(), Error> {
        call_stack.push(decl.ident);
        for &callee in decl.callees.borrow().as_ref().unwrap().iter() {
            let callee_decl = self.shader.find_fn_decl(callee).unwrap();
            if match kind {
                ShaderKind::Vertex => callee_decl.is_used_in_vertex_shader.get().unwrap(),
                ShaderKind::Fragment => callee_decl.is_used_in_fragment_shader.get().unwrap(),
            } {
                continue;
            }
            if call_stack.contains(&callee) {
                return Err(Error {
                    span: decl.span,
                    message: format!("function `{}` recursively calls `{}`", decl.ident, callee),
                });
            }
            self.analyse_call_tree(kind, call_stack, callee_decl)?;
        }
        call_stack.pop();
        match kind {
            ShaderKind::Vertex => decl.is_used_in_vertex_shader.set(Some(true)),
            ShaderKind::Fragment => decl.is_used_in_fragment_shader.set(Some(true)),
        }
        Ok(())
    }

    fn propagate_deps(&mut self, visited: &mut HashSet<Ident>, decl: &FnDecl) -> Result<(), Error> {
        if visited.contains(&decl.ident) {
            return Ok(());
        }
        for &callee in decl.callees.borrow().as_ref().unwrap().iter() {
            let callee_decl = self.shader.find_fn_decl(callee).unwrap();
            self.propagate_deps(visited, callee_decl)?;
            decl.uniform_block_deps
                .borrow_mut()
                .as_mut()
                .unwrap()
                .extend(callee_decl.uniform_block_deps.borrow().as_ref().unwrap());
            decl.has_texture_deps.set(Some(
                decl.has_texture_deps.get().unwrap() || callee_decl.has_texture_deps.get().unwrap(),
            ));
            decl.geometry_deps
                .borrow_mut()
                .as_mut()
                .unwrap()
                .extend(callee_decl.geometry_deps.borrow().as_ref().unwrap());
            decl.instance_deps
                .borrow_mut()
                .as_mut()
                .unwrap()
                .extend(callee_decl.instance_deps.borrow().as_ref().unwrap());
            decl.has_varying_deps.set(Some(
                decl.has_varying_deps.get().unwrap() || callee_decl.has_varying_deps.get().unwrap(),
            ));
            decl.builtin_deps.borrow_mut().as_mut().unwrap().extend(
                callee_decl
                    .builtin_deps
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .iter()
                    .cloned(),
            );
            decl.cons_fn_deps.borrow_mut().as_mut().unwrap().extend(
                callee_decl
                    .cons_fn_deps
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .iter()
                    .cloned(),
            );
        }
        if decl.is_used_in_vertex_shader.get().unwrap()
            && decl.is_used_in_fragment_shader.get().unwrap()
        {
            if !decl.geometry_deps.borrow().as_ref().unwrap().is_empty() {
                return Err(Error {
                    span: decl.span,
                    message: format!(
                        "function `{}` can't access any geometries, since it's used in both the vertex and fragment shader",
                        decl.ident
                    ),
                });
            }
            if !decl.instance_deps.borrow().as_ref().unwrap().is_empty() {
                return Err(Error {
                    span: decl.span,
                    message: format!(
                        "function `{}` can't access any instances, since it's used in both the vertex and fragment shader",
                        decl.ident
                    ),
                });
            }
            if decl.has_varying_deps.get().unwrap() {
                return Err(Error {
                    span: decl.span,
                    message: format!(
                        "function `{}` can't access any varyings, since it's used in both the vertex and fragment shader",
                        decl.ident
                    ),
                });
            }
        }
        visited.insert(decl.ident);
        Ok(())
    }
}

#[derive(Debug)]
struct FnDefAnalyser<'a> {
    builtins: &'a HashMap<Ident, Builtin>,
    shader: &'a ShaderAst,
    decl: &'a FnDecl,
    env: &'a mut Env,
    gather_all: bool,
    is_inside_loop: bool,
}

impl<'a> FnDefAnalyser<'a> {
    fn ty_checker(&self) -> TyChecker {
        TyChecker {
            builtins: self.builtins,
            shader: self.shader,
            env: &self.env,
        }
    }

    fn const_evaluator(&self) -> ConstEvaluator {
        ConstEvaluator {
            shader: self.shader,
        }
    }

    fn const_gatherer(&self) -> ConstGatherer {
        ConstGatherer {
            shader: self.shader,
            gather_all: self.gather_all,
        }
    }

    fn dep_analyser(&self) -> DepAnalyser {
        DepAnalyser {
            shader: self.shader,
            decl: self.decl,
            env: &self.env,
        }
    }

    fn analyse_fn_def(&mut self) -> Result<(), Error> {
        self.env.push_scope();
        for param in &self.decl.params {
            self.env.insert_sym(
                param.span,
                param.ident,
                Sym::Var {
                    is_mut: true,
                    ty: param.ty_expr.ty.borrow().as_ref().unwrap().clone(),
                    kind: VarKind::Local,
                },
            )?;
        }
        *self.decl.return_ty.borrow_mut() = Some(
            self.decl
                .return_ty_expr
                .as_ref()
                .map(|return_ty_expr| return_ty_expr.ty.borrow().as_ref().unwrap().clone())
                .unwrap_or(Ty::Void),
        );
        *self.decl.callees.borrow_mut() = Some(BTreeSet::new());
        *self.decl.uniform_block_deps.borrow_mut() = Some(BTreeSet::new());
        self.decl.has_texture_deps.set(Some(false));
        *self.decl.geometry_deps.borrow_mut() = Some(BTreeSet::new());
        *self.decl.instance_deps.borrow_mut() = Some(BTreeSet::new());
        self.decl.has_varying_deps.set(Some(false));
        *self.decl.builtin_deps.borrow_mut() = Some(BTreeSet::new());
        *self.decl.cons_fn_deps.borrow_mut() = Some(BTreeSet::new());
        self.analyse_block(&self.decl.block)?;
        self.env.pop_scope();
        Ok(())
    }

    fn analyse_block(&mut self, block: &Block) -> Result<(), Error> {
        for stmt in &block.stmts {
            self.analyse_stmt(stmt)?;
        }
        Ok(())
    }

    fn analyse_stmt(&mut self, stmt: &Stmt) -> Result<(), Error> {
        match *stmt {
            Stmt::Break { span } => self.analyse_break_stmt(span),
            Stmt::Continue { span } => self.analyse_continue_stmt(span),
            Stmt::For {
                span,
                ident,
                ref from_expr,
                ref to_expr,
                ref step_expr,
                ref block,
            } => self.analyse_for_stmt(span, ident, from_expr, to_expr, step_expr, block),
            Stmt::If {
                span,
                ref expr,
                ref block_if_true,
                ref block_if_false,
            } => self.analyse_if_stmt(span, expr, block_if_true, block_if_false),
            Stmt::Let {
                span,
                ref ty,
                ident,
                ref ty_expr,
                ref expr,
            } => self.analyse_let_stmt(span, ty, ident, ty_expr, expr),
            Stmt::Return { span, ref expr } => self.analyse_return_stmt(span, expr),
            Stmt::Block { span, ref block } => self.analyse_block_stmt(span, block),
            Stmt::Expr { span, ref expr } => self.analyse_expr_stmt(span, expr),
        }
    }

    fn analyse_break_stmt(&self, span: Span) -> Result<(), Error> {
        if !self.is_inside_loop {
            return Err(Error {
                span,
                message: String::from("break outside loop"),
            }
            .into());
        }
        Ok(())
    }

    fn analyse_continue_stmt(&self, span: Span) -> Result<(), Error> {
        if !self.is_inside_loop {
            return Err(Error {
                span,
                message: String::from("continue outside loop"),
            }
            .into());
        }
        Ok(())
    }

    fn analyse_for_stmt(
        &mut self,
        span: Span,
        ident: Ident,
        from_expr: &Expr,
        to_expr: &Expr,
        step_expr: &Option<Expr>,
        block: &Block,
    ) -> Result<(), Error> {
        self.ty_checker()
            .ty_check_expr_with_expected_ty(span, from_expr, &Ty::Int)?;
        let from = self
            .const_evaluator()
            .const_eval_expr(from_expr)?
            .to_int()
            .unwrap();
        self.dep_analyser().dep_analyse_expr(from_expr);
        self.ty_checker()
            .ty_check_expr_with_expected_ty(span, to_expr, &Ty::Int)?;
        let to = self
            .const_evaluator()
            .const_eval_expr(to_expr)?
            .to_int()
            .unwrap();
        self.dep_analyser().dep_analyse_expr(to_expr);
        if let Some(step_expr) = step_expr {
            self.ty_checker()
                .ty_check_expr_with_expected_ty(span, step_expr, &Ty::Int)?;
            let step = self
                .const_evaluator()
                .const_eval_expr(step_expr)?
                .to_int()
                .unwrap();
            if step == 0 {
                return Err(Error {
                    span,
                    message: String::from("step must not be zero"),
                }
                .into());
            }
            if from < to && step < 0 {
                return Err(Error {
                    span,
                    message: String::from("step must not be positive"),
                }
                .into());
            }
            if from > to && step > 0 {
                return Err(Error {
                    span,
                    message: String::from("step must not be negative"),
                }
                .into());
            }
            self.dep_analyser().dep_analyse_expr(step_expr);
        }
        self.env.push_scope();
        self.env.insert_sym(
            span,
            ident,
            Sym::Var {
                is_mut: false,
                ty: Ty::Int,
                kind: VarKind::Local,
            },
        )?;
        let was_inside_loop = self.is_inside_loop;
        self.is_inside_loop = true;
        self.analyse_block(block)?;
        self.is_inside_loop = was_inside_loop;
        self.env.pop_scope();
        Ok(())
    }

    fn analyse_if_stmt(
        &mut self,
        span: Span,
        expr: &Expr,
        block_if_true: &Block,
        block_if_false: &Option<Box<Block>>,
    ) -> Result<(), Error> {
        self.ty_checker()
            .ty_check_expr_with_expected_ty(span, expr, &Ty::Bool)?;
        self.const_evaluator().try_const_eval_expr(expr);
        self.const_gatherer().const_gather_expr(expr);
        self.dep_analyser().dep_analyse_expr(expr);
        self.env.push_scope();
        self.analyse_block(block_if_true)?;
        self.env.pop_scope();
        if let Some(block_if_false) = block_if_false {
            self.env.push_scope();
            self.analyse_block(block_if_false)?;
            self.env.pop_scope();
        }
        Ok(())
    }

    fn analyse_let_stmt(
        &mut self,
        span: Span,
        ty: &RefCell<Option<Ty>>,
        ident: Ident,
        ty_expr: &Option<TyExpr>,
        expr: &Option<Expr>,
    ) -> Result<(), Error> {
        *ty.borrow_mut() = Some(if let Some(ty_expr) = ty_expr {
            let expected_ty = self.ty_checker().ty_check_ty_expr(ty_expr)?;
            if let Some(expr) = expr {
                let actual_ty =
                    self.ty_checker()
                        .ty_check_expr_with_expected_ty(span, expr, &expected_ty)?;
                self.dep_analyser().dep_analyse_expr(expr);
                actual_ty
            } else {
                expected_ty
            }
        } else if let Some(expr) = expr {
            let ty = self.ty_checker().ty_check_expr(expr)?;
			if ty == Ty::Void {
				return Err(Error {
					span,
					message: String::from("init expression cannot be void")
				});
			}
            self.const_evaluator().try_const_eval_expr(expr);
            self.const_gatherer().const_gather_expr(expr);
            self.dep_analyser().dep_analyse_expr(expr);
            ty
        } else {
            return Err(Error {
                span,
                message: format!("can't infer type of variable `{}`", ident),
            });
        });
        self.env.insert_sym(
            span,
            ident,
            Sym::Var {
                is_mut: true,
                ty: ty.borrow().as_ref().unwrap().clone(),
                kind: VarKind::Local,
            },
        )
    }

    fn analyse_return_stmt(&mut self, span: Span, expr: &Option<Expr>) -> Result<(), Error> {
        
        
        if let Some(expr) = expr {
            self.ty_checker().ty_check_expr_with_expected_ty(
                span,
                expr,
                self.decl.return_ty.borrow().as_ref().unwrap(),
            )?;

            self.const_evaluator().try_const_eval_expr(expr);
            self.const_gatherer().const_gather_expr(expr);
            self.dep_analyser().dep_analyse_expr(expr);
        } else if self.decl.return_ty.borrow().as_ref().unwrap() != &Ty::Void {
            return Err(Error {
                span,
                message: String::from("missing return expression"),
            }
            .into());
        }
        Ok(())
    }

    fn analyse_block_stmt(&mut self, _span: Span, block: &Block) -> Result<(), Error> {
        self.env.push_scope();
        self.analyse_block(block)?;
        self.env.pop_scope();
        Ok(())
    }

    fn analyse_expr_stmt(&mut self, _span: Span, expr: &Expr) -> Result<(), Error> {
        self.ty_checker().ty_check_expr(expr)?;
        self.const_evaluator().try_const_eval_expr(expr);
        self.const_gatherer().const_gather_expr(expr);
        self.dep_analyser().dep_analyse_expr(expr);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
enum ShaderKind {
    Vertex,
    Fragment,
}
