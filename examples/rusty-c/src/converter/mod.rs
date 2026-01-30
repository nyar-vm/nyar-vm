use crate::ast as mini_ast;
use chomsky_source::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, Id, IntentBuilder};
use oak_c::CElementType;
use oak_c::CLanguage;
use oak_c::CTokenType;
use oak_core::tree::{RedNode, RedTree};

/// 将 RedNode 转换为 UIR (Intent Graph)
pub fn red_to_uir<'a>(
    node: &RedNode<'a, CLanguage>,
    source: &str,
    builder: &mut IntentBuilder<ConstraintAnalysis>,
) -> Id {
    let span = node.span();
    let loc = Loc::new(0, span.start as u32, span.end as u32);

    match node.green.kind() {
        CElementType::Root => {
            let mut items = Vec::new();
            for child in node.children() {
                if let RedTree::Node(n) = child {
                    let id = red_to_uir(&n, source, builder);
                    items.push(id);
                }
            }
            // Root is effectively a sequence of items (Module)
            builder.extension("module", items, loc)
        }
        CElementType::FunctionDefinition => {
            let mut name = "unknown_func".to_string();
            let mut body_id = builder.constant(0, loc); // placeholder

            for child in node.children() {
                match child {
                    RedTree::Leaf(l) if l.kind == CTokenType::Identifier => {
                        name = source[l.span.start as usize..l.span.end as usize].to_string();
                    }
                    RedTree::Node(n) if n.green.kind() == CElementType::CompoundStatement => {
                        body_id = red_to_uir(&n, source, builder);
                    }
                    _ => {}
                }
            }

            // Function definition in IKun: StateUpdate(Symbol(name), Lambda([], body))
            // Assuming no params for now as per original code
            let lambda = builder.lambda(vec![], body_id, loc);
            builder.assign(&name, lambda, loc)
        }
        CElementType::CompoundStatement => {
            let mut stmts = Vec::new();
            for child in node.children() {
                if let RedTree::Node(n) = child {
                    stmts.push(red_to_uir(&n, source, builder));
                }
            }
            builder.seq(stmts, loc)
        }
        CElementType::ReturnStatement => {
            let mut args = Vec::new();
            for child in node.children() {
                if let RedTree::Node(n) = child {
                    args.push(red_to_uir(&n, source, builder));
                }
            }
            if let Some(arg) = args.first() {
                builder.return_(*arg, loc)
            } else {
                let void = builder.constant(0, loc);
                builder.return_(void, loc)
            }
        }
        CElementType::ExpressionStatement | CElementType::DeclarationStatement => {
            for child in node.children() {
                if let RedTree::Node(n) = child {
                    return red_to_uir(&n, source, builder);
                }
            }
            // Empty statement?
            builder.constant(0, loc)
        }
        _ => {
            // Check for literals
            let text = source[span.start as usize..span.end as usize].to_string();
            if let Ok(i) = text.parse::<i64>() {
                return builder.constant(i, loc);
            }
            // Check for identifiers
            if node.green.kind() == CElementType::Token(CTokenType::Identifier) {
                return builder.symbol(&text, loc);
            }

            // Recurse for others (e.g. Binary Expr)
            // Note: The original parser logic for binary exprs wasn't explicitly shown in the snippet
            // except the "fallback" loop. Assuming RedNode structure for binary ops.
            // But wait, the original code fell back to "children_nodes".

            let mut children_nodes = Vec::new();
            for child in node.children() {
                if let RedTree::Node(n) = child {
                    children_nodes.push(red_to_uir(&n, source, builder));
                }
            }

            if children_nodes.is_empty() {
                // Literal string or identifier if not parsed above
                if text.starts_with('"') {
                    builder.string(&text, loc)
                } else {
                    builder.symbol(&text, loc)
                }
            } else if children_nodes.len() == 1 {
                children_nodes[0]
            } else {
                // Tuple or implicit sequence? 
                builder.extension("tuple", children_nodes, loc)
            }
        }
    }
}

// --- UIR to MiniC AST ---

pub fn uir_to_minic(egraph: &EGraph<IKun, ConstraintAnalysis>, root: Id) -> mini_ast::Program {
    let mut declarations = Vec::new();

    // Assume root is the Module (Extension "module")
    let class = egraph.get_class(root);
    let node = &class.nodes[0];

    if let IKun::Extension(name, items) = node {
        if name == "module" {
            for &item in items {
                if let Some(decl) = uir_to_declaration(egraph, item) {
                    declarations.push(decl);
                }
            }
        } else {
            // Single item program?
            if let Some(decl) = uir_to_declaration(egraph, root) {
                declarations.push(decl);
            }
        }
    } else if let IKun::Seq(items) = node {
        for &item in items {
            if let Some(decl) = uir_to_declaration(egraph, item) {
                declarations.push(decl);
            }
        }
    }

    mini_ast::Program { declarations }
}

fn uir_to_declaration(
    egraph: &EGraph<IKun, ConstraintAnalysis>,
    id: Id,
) -> Option<mini_ast::Declaration> {
    let class = egraph.get_class(id);
    let node = &class.nodes[0];

    match node {
        IKun::StateUpdate(target, value) => {
            // Variable or Function definition
            // target should be Symbol
            let target_node = &egraph.get_class(*target).nodes[0];
            let name = if let IKun::Symbol(s) = target_node {
                s.clone()
            } else {
                "unknown".to_string()
            };

            let value_node = &egraph.get_class(*value).nodes[0];
            match value_node {
                IKun::Lambda(params, body) => {
                    // Function
                    let body_stmt = uir_to_compound_statement(egraph, *body);
                    Some(mini_ast::Declaration::Function {
                        return_type: mini_ast::Type::Basic(mini_ast::BasicType::Int),
                        name,
                        parameters: params
                            .iter()
                            .map(|p| mini_ast::Parameter {
                                type_: mini_ast::Type::Basic(mini_ast::BasicType::Int),
                                name: Some(p.clone()),
                            })
                            .collect(),
                        body: Some(body_stmt),
                    })
                }
                _ => {
                    // Variable
                    Some(mini_ast::Declaration::Variable {
                        type_: mini_ast::Type::Basic(mini_ast::BasicType::Int),
                        name,
                        initializer: Some(uir_to_expression(egraph, *value)),
                    })
                }
            }
        }
        _ => None,
    }
}

fn uir_to_compound_statement(
    egraph: &EGraph<IKun, ConstraintAnalysis>,
    id: Id,
) -> mini_ast::CompoundStatement {
    let class = egraph.get_class(id);
    let node = &class.nodes[0];

    let stmts = match node {
        IKun::Seq(items) => items.iter().map(|&i| uir_to_statement(egraph, i)).collect(),
        _ => vec![uir_to_statement(egraph, id)],
    };

    mini_ast::CompoundStatement { statements: stmts }
}

fn uir_to_statement(egraph: &EGraph<IKun, ConstraintAnalysis>, id: Id) -> mini_ast::Statement {
    let class = egraph.get_class(id);
    let node = &class.nodes[0];

    match node {
        IKun::Extension(name, args) if name == "return" => {
            mini_ast::Statement::Return(args.first().map(|&a| uir_to_expression(egraph, a)))
        }
        IKun::StateUpdate(target, value) => {
            let target_expr = uir_to_expression(egraph, *target);
            let value_expr = uir_to_expression(egraph, *value);
            mini_ast::Statement::Expression(Some(mini_ast::Expression::Assignment {
                left: Box::new(target_expr),
                operator: mini_ast::AssignmentOperator::Assign,
                right: Box::new(value_expr),
            }))
        }
        IKun::Seq(_) => mini_ast::Statement::Compound(uir_to_compound_statement(egraph, id)),
        _ => mini_ast::Statement::Expression(Some(uir_to_expression(egraph, id))),
    }
}

fn uir_to_expression(egraph: &EGraph<IKun, ConstraintAnalysis>, id: Id) -> mini_ast::Expression {
    let class = egraph.get_class(id);
    let node = &class.nodes[0];

    match node {
        IKun::Constant(i) => mini_ast::Expression::Literal(mini_ast::Literal::Integer(*i)),
        IKun::FloatConstant(f) => {
            mini_ast::Expression::Literal(mini_ast::Literal::Float(f64::from_bits(*f)))
        }
        IKun::StringConstant(s) => {
            mini_ast::Expression::Literal(mini_ast::Literal::String(s.clone()))
        }
        IKun::Symbol(s) => mini_ast::Expression::Identifier(s.clone()),
        IKun::Extension(name, args) => {
            // Handle binary ops if we mapped them to extensions
            let op = match name.as_str() {
                "Add" => Some(mini_ast::BinaryOperator::Add),
                "Subtract" => Some(mini_ast::BinaryOperator::Subtract),
                "Multiply" => Some(mini_ast::BinaryOperator::Multiply),
                "Divide" => Some(mini_ast::BinaryOperator::Divide),
                _ => None,
            };

            if let Some(binary_op) = op {
                if args.len() == 2 {
                    return mini_ast::Expression::Binary {
                        left: Box::new(uir_to_expression(egraph, args[0])),
                        operator: binary_op,
                        right: Box::new(uir_to_expression(egraph, args[1])),
                    };
                }
            }

            // Fallback to tuple or unknown
            if name == "tuple" && !args.is_empty() {
                uir_to_expression(egraph, args[0])
            } else {
                mini_ast::Expression::Identifier(name.clone())
            }
        }
        IKun::Apply(func, args) => {
            let func_expr = uir_to_expression(egraph, *func);
            mini_ast::Expression::Call {
                function: Box::new(func_expr),
                arguments: args.iter().map(|&a| uir_to_expression(egraph, a)).collect(),
            }
        }
        _ => mini_ast::Expression::Identifier("unsupported".to_string()),
    }
}
