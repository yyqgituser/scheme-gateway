use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::execution_engine::JitFunction;
use inkwell::module::Module;
use inkwell::types::StructType;
use inkwell::values::{AnyValue, BasicMetadataValueEnum, FunctionValue, IntValue, StructValue};
use inkwell::IntPredicate;
use inkwell::OptimizationLevel;

use std::collections::HashMap;

use crate::ast::Expr;
use crate::runtime;

const TAG_INT: u64 = 0;
const TAG_BOOL: u64 = 1;
const TAG_NIL: u64 = 2;

pub struct Codegen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    tagged_type: StructType<'ctx>,
    functions: HashMap<String, FunctionValue<'ctx>>,
}

impl<'ctx> Codegen<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let module = context.create_module("tiny_scheme");
        let builder = context.create_builder();
        let tagged_type = context.struct_type(
            &[context.i8_type().into(), context.i64_type().into()],
            false,
        );
        let mut cg = Codegen {
            context, module, builder, tagged_type,
            functions: HashMap::new(),
        };
        cg.declare_runtime_functions();
        cg
    }

    fn declare_runtime_functions(&mut self) {
        let void_type = self.context.void_type();
        let fn_type = void_type.fn_type(
            &[self.context.i8_type().into(), self.context.i64_type().into()],
            false,
        );
        self.module.add_function("rt_print_value", fn_type, None);
    }

    // --- Tagged Value helpers ---

    fn pack_tagged(&self, tag: u64, payload: IntValue<'ctx>) -> StructValue<'ctx> {
        let tag_val = self.context.i8_type().const_int(tag, false);
        let v = self.tagged_type.const_zero();
        let v = self.builder.build_insert_value(v, tag_val, 0, "")
            .unwrap().into_struct_value();
        self.builder.build_insert_value(v, payload, 1, "")
            .unwrap().into_struct_value()
    }

    fn make_tagged_int(&self, n: i64) -> StructValue<'ctx> {
        let payload = self.context.i64_type().const_int(n as u64, false);
        self.pack_tagged(TAG_INT, payload)
    }

    fn make_tagged_bool(&self, b: bool) -> StructValue<'ctx> {
        let payload = self.context.i64_type().const_int(if b { 1 } else { 0 }, false);
        self.pack_tagged(TAG_BOOL, payload)
    }

    fn make_tagged_nil(&self) -> StructValue<'ctx> {
        let payload = self.context.i64_type().const_int(0, false);
        self.pack_tagged(TAG_NIL, payload)
    }

    fn extract_payload(&self, val: StructValue<'ctx>) -> IntValue<'ctx> {
        self.builder.build_extract_value(val, 1, "payload")
            .unwrap().into_int_value()
    }

    // --- Expression compilation ---

    fn compile_expr(
        &self,
        expr: &Expr,
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        match expr {
            Expr::Int(n) => self.make_tagged_int(*n),
            Expr::Bool(b) => self.make_tagged_bool(*b),
            Expr::Nil => self.make_tagged_nil(),
            Expr::Symbol(name) => {
                *locals.get(name.as_str())
                    .unwrap_or_else(|| panic!("JIT: undefined symbol: {}", name))
            }
            Expr::List(elems) => self.compile_list(elems, locals, current_fn),
            Expr::Str(_) => panic!("JIT: string type not supported"),
        }
    }

    fn compile_list(
        &self,
        elems: &[Expr],
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        if elems.is_empty() {
            return self.make_tagged_nil();
        }

        match &elems[0] {
            Expr::Symbol(name) => match name.as_str() {
                "if" => self.compile_if(elems, locals, current_fn),
                "print" => self.compile_print(elems, locals, current_fn),
                "begin" => self.compile_begin(elems, locals, current_fn),
                "let" => self.compile_let(elems, locals, current_fn),
                "not" => self.compile_not(elems, locals, current_fn),
                "cond" => self.compile_cond(elems, locals, current_fn),
                "and" => self.compile_and(elems, locals, current_fn),
                "or" => self.compile_or(elems, locals, current_fn),
                "+" | "-" | "*" | "/" | "%" => self.compile_arith(name, elems, locals, current_fn),
                "=" | "<" | ">" | "<=" | ">=" => self.compile_cmp(name, elems, locals, current_fn),
                _ => self.compile_user_call(name, elems, locals, current_fn),
            },
            _ => panic!("JIT: unsupported call form: {:?}", elems[0]),
        }
    }

    fn compile_print(
        &self,
        elems: &[Expr],
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        let val = self.compile_expr(&elems[1], locals, current_fn);
        let tag = self.builder.build_extract_value(val, 0, "tag")
            .unwrap().into_int_value();
        let payload = self.extract_payload(val);
        let print_fn = self.module.get_function("rt_print_value").unwrap();
        self.builder.build_call(
            print_fn,
            &[tag.into(), payload.into()],
            "",
        ).unwrap();
        self.make_tagged_nil()
    }

    fn compile_arith(
        &self,
        op: &str,
        elems: &[Expr],
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        let lhs = self.compile_expr(&elems[1], locals, current_fn);
        let rhs = self.compile_expr(&elems[2], locals, current_fn);
        let l = self.extract_payload(lhs);
        let r = self.extract_payload(rhs);
        let result = match op {
            "+" => self.builder.build_int_add(l, r, "add").unwrap(),
            "-" => self.builder.build_int_sub(l, r, "sub").unwrap(),
            "*" => self.builder.build_int_mul(l, r, "mul").unwrap(),
            "/" => self.builder.build_int_signed_div(l, r, "div").unwrap(),
            "%" => self.builder.build_int_signed_rem(l, r, "rem").unwrap(),
            _ => unreachable!(),
        };
        self.pack_tagged(TAG_INT, result)
    }

    fn compile_cmp(
        &self,
        op: &str,
        elems: &[Expr],
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        let lhs = self.compile_expr(&elems[1], locals, current_fn);
        let rhs = self.compile_expr(&elems[2], locals, current_fn);
        let l = self.extract_payload(lhs);
        let r = self.extract_payload(rhs);
        let pred = match op {
            "=" => IntPredicate::EQ,
            "<" => IntPredicate::SLT,
            ">" => IntPredicate::SGT,
            "<=" => IntPredicate::SLE,
            ">=" => IntPredicate::SGE,
            _ => unreachable!(),
        };
        let cmp = self.builder.build_int_compare(pred, l, r, "cmp").unwrap();
        let result = self.builder.build_int_z_extend(
            cmp, self.context.i64_type(), "bool_ext",
        ).unwrap();
        self.pack_tagged(TAG_BOOL, result)
    }

    fn compile_if(
        &self,
        elems: &[Expr],
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        let cond = self.compile_expr(&elems[1], locals, current_fn);
        let cond_payload = self.extract_payload(cond);
        let cond_i1 = self.builder.build_int_compare(
            IntPredicate::NE,
            cond_payload,
            self.context.i64_type().const_int(0, false),
            "cond",
        ).unwrap();

        let then_bb = self.context.append_basic_block(current_fn, "then");
        let else_bb = self.context.append_basic_block(current_fn, "else");
        let merge_bb = self.context.append_basic_block(current_fn, "merge");

        self.builder.build_conditional_branch(cond_i1, then_bb, else_bb).unwrap();

        // Then branch — compile may create new blocks, so capture end block
        self.builder.position_at_end(then_bb);
        let then_val = self.compile_expr(&elems[2], locals, current_fn);
        let then_end_bb = self.builder.get_insert_block().unwrap();
        self.builder.build_unconditional_branch(merge_bb).unwrap();

        // Else branch
        self.builder.position_at_end(else_bb);
        let else_val = self.compile_expr(&elems[3], locals, current_fn);
        let else_end_bb = self.builder.get_insert_block().unwrap();
        self.builder.build_unconditional_branch(merge_bb).unwrap();

        // Merge with phi
        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(self.tagged_type, "if_result").unwrap();
        phi.add_incoming(&[
            (&then_val, then_end_bb),
            (&else_val, else_end_bb),
        ]);
        phi.as_basic_value().into_struct_value()
    }

    fn compile_user_call(
        &self,
        name: &str,
        elems: &[Expr],
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        let function = self.functions.get(name)
            .unwrap_or_else(|| panic!("JIT: undefined function: {}", name));
        let args: Vec<BasicMetadataValueEnum> = elems[1..]
            .iter()
            .map(|e| self.compile_expr(e, locals, current_fn).into())
            .collect();
        self.builder
            .build_call(*function, &args, "call")
            .unwrap()
            .as_any_value_enum()
            .into_struct_value()
    }

    fn compile_begin(
        &self,
        elems: &[Expr],
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        let mut result = self.make_tagged_nil();
        for expr in &elems[1..] {
            result = self.compile_expr(expr, locals, current_fn);
        }
        result
    }

    fn compile_let(
        &self,
        elems: &[Expr],
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        let bindings = match &elems[1] {
            Expr::List(pairs) => pairs,
            _ => panic!("JIT: let: expected bindings list"),
        };
        let mut child_locals = locals.clone();
        for pair in bindings {
            match pair {
                Expr::List(kv) => {
                    let name = match &kv[0] {
                        Expr::Symbol(s) => s.clone(),
                        _ => panic!("JIT: let: expected symbol in binding"),
                    };
                    // Bindings evaluate in the parent env, not the child
                    let val = self.compile_expr(&kv[1], locals, current_fn);
                    child_locals.insert(name, val);
                }
                _ => panic!("JIT: let: expected (name value) pair"),
            }
        }
        self.compile_expr(&elems[2], &child_locals, current_fn)
    }

    fn compile_not(
        &self,
        elems: &[Expr],
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        let val = self.compile_expr(&elems[1], locals, current_fn);
        let tag = self.builder.build_extract_value(val, 0, "tag")
            .unwrap().into_int_value();
        let payload = self.extract_payload(val);

        // Falsy = (tag == BOOL && payload == 0) || (tag == NIL)
        let is_bool = self.builder.build_int_compare(
            IntPredicate::EQ, tag,
            self.context.i8_type().const_int(TAG_BOOL, false), "is_bool",
        ).unwrap();
        let is_zero = self.builder.build_int_compare(
            IntPredicate::EQ, payload,
            self.context.i64_type().const_int(0, false), "is_zero",
        ).unwrap();
        let is_bool_false = self.builder.build_and(is_bool, is_zero, "is_bool_false").unwrap();
        let is_nil = self.builder.build_int_compare(
            IntPredicate::EQ, tag,
            self.context.i8_type().const_int(TAG_NIL, false), "is_nil",
        ).unwrap();
        let is_falsy = self.builder.build_or(is_bool_false, is_nil, "is_falsy").unwrap();

        let result = self.builder.build_int_z_extend(
            is_falsy, self.context.i64_type(), "not_ext",
        ).unwrap();
        self.pack_tagged(TAG_BOOL, result)
    }

    /// Build an i1 representing whether a tagged value is falsy.
    /// Falsy = (tag==BOOL && payload==0) || tag==NIL
    fn build_is_falsy(&self, val: StructValue<'ctx>) -> IntValue<'ctx> {
        let tag = self.builder.build_extract_value(val, 0, "tag")
            .unwrap().into_int_value();
        let payload = self.extract_payload(val);

        let is_bool = self.builder.build_int_compare(
            IntPredicate::EQ, tag,
            self.context.i8_type().const_int(TAG_BOOL, false), "is_bool",
        ).unwrap();
        let is_zero = self.builder.build_int_compare(
            IntPredicate::EQ, payload,
            self.context.i64_type().const_int(0, false), "is_zero",
        ).unwrap();
        let is_bool_false = self.builder.build_and(is_bool, is_zero, "is_bool_false").unwrap();
        let is_nil = self.builder.build_int_compare(
            IntPredicate::EQ, tag,
            self.context.i8_type().const_int(TAG_NIL, false), "is_nil",
        ).unwrap();
        self.builder.build_or(is_bool_false, is_nil, "is_falsy").unwrap()
    }

    fn compile_cond(
        &self,
        elems: &[Expr],
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        // (cond (test1 body1) (test2 body2) ... (else bodyN))
        // Compile as chained if-else basic blocks
        let merge_bb = self.context.append_basic_block(current_fn, "cond_merge");
        let phi_type = self.tagged_type;

        // Collect (value, source_block) pairs for the phi node
        let mut incoming: Vec<(StructValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> = Vec::new();

        for clause in &elems[1..] {
            let parts = match clause {
                Expr::List(p) => p,
                _ => panic!("JIT: cond: expected clause list"),
            };

            // Check for else clause
            if let Expr::Symbol(s) = &parts[0] {
                if s == "else" {
                    let val = self.compile_expr(&parts[1], locals, current_fn);
                    let src_bb = self.builder.get_insert_block().unwrap();
                    incoming.push((val, src_bb));
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                    // Position at merge and build phi
                    self.builder.position_at_end(merge_bb);
                    let phi = self.builder.build_phi(phi_type, "cond_result").unwrap();
                    for (val, bb) in &incoming {
                        phi.add_incoming(&[(val, *bb)]);
                    }
                    return phi.as_basic_value().into_struct_value();
                }
            }

            // Normal clause: test → body or fall through
            let test_val = self.compile_expr(&parts[0], locals, current_fn);
            let is_falsy = self.build_is_falsy(test_val);

            let body_bb = self.context.append_basic_block(current_fn, "cond_body");
            let next_bb = self.context.append_basic_block(current_fn, "cond_next");

            self.builder.build_conditional_branch(is_falsy, next_bb, body_bb).unwrap();

            // Body
            self.builder.position_at_end(body_bb);
            let body_val = self.compile_expr(&parts[1], locals, current_fn);
            let body_end_bb = self.builder.get_insert_block().unwrap();
            incoming.push((body_val, body_end_bb));
            self.builder.build_unconditional_branch(merge_bb).unwrap();

            // Continue to next clause
            self.builder.position_at_end(next_bb);
        }

        // No else clause: fall through produces nil
        let nil_val = self.make_tagged_nil();
        let nil_bb = self.builder.get_insert_block().unwrap();
        incoming.push((nil_val, nil_bb));
        self.builder.build_unconditional_branch(merge_bb).unwrap();

        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(phi_type, "cond_result").unwrap();
        for (val, bb) in &incoming {
            phi.add_incoming(&[(val, *bb)]);
        }
        phi.as_basic_value().into_struct_value()
    }

    fn compile_and(
        &self,
        elems: &[Expr],
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        // (and e1 e2 ...) — short-circuit: return first falsy, or last value
        if elems.len() == 1 {
            return self.make_tagged_bool(true);
        }

        let merge_bb = self.context.append_basic_block(current_fn, "and_merge");
        let mut incoming: Vec<(StructValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> = Vec::new();

        for (i, expr) in elems[1..].iter().enumerate() {
            let val = self.compile_expr(expr, locals, current_fn);
            let is_last = i == elems.len() - 2;

            if is_last {
                let src_bb = self.builder.get_insert_block().unwrap();
                incoming.push((val, src_bb));
                self.builder.build_unconditional_branch(merge_bb).unwrap();
            } else {
                let is_falsy = self.build_is_falsy(val);
                let next_bb = self.context.append_basic_block(current_fn, "and_next");
                let src_bb = self.builder.get_insert_block().unwrap();
                incoming.push((val, src_bb));
                self.builder.build_conditional_branch(is_falsy, merge_bb, next_bb).unwrap();
                self.builder.position_at_end(next_bb);
            }
        }

        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(self.tagged_type, "and_result").unwrap();
        for (val, bb) in &incoming {
            phi.add_incoming(&[(val, *bb)]);
        }
        phi.as_basic_value().into_struct_value()
    }

    fn compile_or(
        &self,
        elems: &[Expr],
        locals: &HashMap<String, StructValue<'ctx>>,
        current_fn: FunctionValue<'ctx>,
    ) -> StructValue<'ctx> {
        // (or e1 e2 ...) — short-circuit: return first truthy, or last value
        if elems.len() == 1 {
            return self.make_tagged_bool(false);
        }

        let merge_bb = self.context.append_basic_block(current_fn, "or_merge");
        let mut incoming: Vec<(StructValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> = Vec::new();

        for (i, expr) in elems[1..].iter().enumerate() {
            let val = self.compile_expr(expr, locals, current_fn);
            let is_last = i == elems.len() - 2;

            if is_last {
                let src_bb = self.builder.get_insert_block().unwrap();
                incoming.push((val, src_bb));
                self.builder.build_unconditional_branch(merge_bb).unwrap();
            } else {
                let is_falsy = self.build_is_falsy(val);
                let next_bb = self.context.append_basic_block(current_fn, "or_next");
                let src_bb = self.builder.get_insert_block().unwrap();
                incoming.push((val, src_bb));
                // truthy → merge (short circuit), falsy → try next
                self.builder.build_conditional_branch(is_falsy, next_bb, merge_bb).unwrap();
                self.builder.position_at_end(next_bb);
            }
        }

        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(self.tagged_type, "or_result").unwrap();
        for (val, bb) in &incoming {
            phi.add_incoming(&[(val, *bb)]);
        }
        phi.as_basic_value().into_struct_value()
    }

    // --- Function definition ---

    fn compile_function_def(&self, parts: &[Expr], body: &Expr) {
        let name = match &parts[0] {
            Expr::Symbol(s) => s.as_str(),
            _ => panic!("JIT: expected function name"),
        };
        let function = *self.functions.get(name)
            .unwrap_or_else(|| panic!("JIT: function not declared: {}", name));

        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        let mut locals = HashMap::new();
        for (i, param) in parts[1..].iter().enumerate() {
            let param_name = match param {
                Expr::Symbol(s) => s.clone(),
                _ => panic!("JIT: expected parameter name"),
            };
            let param_val = function
                .get_nth_param(i as u32)
                .unwrap()
                .into_struct_value();
            locals.insert(param_name, param_val);
        }

        let result = self.compile_expr(body, &locals, function);
        self.builder.build_return(Some(&result)).unwrap();
    }

    // --- Top-level entry ---

    pub fn compile_and_run(mut self, exprs: &[Expr]) {
        // Phase 1: Declare all user-defined functions (forward declarations)
        for expr in exprs {
            if let Expr::List(elems) = expr {
                if matches!(elems.first(), Some(Expr::Symbol(s)) if s == "define") {
                    if let Expr::List(parts) = &elems[1] {
                        let name = match &parts[0] {
                            Expr::Symbol(s) => s.clone(),
                            _ => panic!("JIT: expected function name in define"),
                        };
                        let param_count = parts.len() - 1;
                        let param_types: Vec<_> =
                            vec![self.tagged_type.into(); param_count];
                        let fn_type = self.tagged_type.fn_type(&param_types, false);
                        let function = self.module.add_function(&name, fn_type, None);
                        self.functions.insert(name, function);
                    }
                }
            }
        }

        // Phase 2: Compile function bodies
        for expr in exprs {
            if let Expr::List(elems) = expr {
                if matches!(elems.first(), Some(Expr::Symbol(s)) if s == "define") {
                    if let Expr::List(parts) = &elems[1] {
                        self.compile_function_def(parts, &elems[2]);
                    }
                }
            }
        }

        // Phase 3: Build __main with non-define and variable-define top-level expressions
        let main_fn_type = self.context.void_type().fn_type(&[], false);
        let main_fn = self.module.add_function("__main", main_fn_type, None);
        let entry = self.context.append_basic_block(main_fn, "entry");
        self.builder.position_at_end(entry);

        let mut globals: HashMap<String, StructValue> = HashMap::new();
        for expr in exprs {
            if let Expr::List(elems) = expr {
                if matches!(elems.first(), Some(Expr::Symbol(s)) if s == "define") {
                    match &elems[1] {
                        // (define x expr) — global variable
                        Expr::Symbol(name) => {
                            let val = self.compile_expr(&elems[2], &globals, main_fn);
                            globals.insert(name.clone(), val);
                            continue;
                        }
                        // (define (f ...) body) — already compiled in Phase 2
                        Expr::List(_) => continue,
                        _ => panic!("JIT: invalid define"),
                    }
                }
            }
            self.compile_expr(expr, &globals, main_fn);
        }
        self.builder.build_return(None).unwrap();

        // Phase 4: JIT execute
        let rt_print = self.module.get_function("rt_print_value").unwrap();
        let ee = self.module
            .create_jit_execution_engine(OptimizationLevel::None)
            .expect("Failed to create JIT execution engine");
        ee.add_global_mapping(
            &rt_print,
            runtime::rt_print_value as *const () as usize,
        );

        unsafe {
            type MainFunc = unsafe extern "C" fn();
            let jit_main: JitFunction<MainFunc> = ee
                .get_function("__main")
                .expect("Failed to find __main");
            jit_main.call();
        }
    }
}
