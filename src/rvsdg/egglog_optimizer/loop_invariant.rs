use super::{BrilOp, BRIL_OPS};

fn inv_ops_detection(bril_op: BrilOp) -> String {
    let bop = bril_op.op.to_string();
    match bril_op.input_types.as_ref() {
        [Some(_), Some(_)] => format!(
            "(rule ((find_inv_expr theta ({bop} ty a b)))
                ((find_inv_operand theta a) (find_inv_operand theta b)) :ruleset fast-analyses)
                
            (rule ((= true (is_inv_operand body a))
                    (= true (is_inv_operand body b))
                    (find_inv_expr body expr)
                    (= expr ({bop} ty a b)))
                ((set (is_inv_expr body expr) true)) :ruleset fast-analyses)
                    "
        ),

        [Some(_), None] => format!(
            "(rule ((find_inv_expr theta ({bop} ty a)))
                ((find_inv_operand theta a)) :ruleset fast-analyses)
                
            (rule ((= true (is_inv_operand body a))
                    (find_inv_expr body expr)
                    (= expr ({bop} ty a)))
                ((set (is_inv_expr body expr) true)) :ruleset fast-analyses)"
        ),
        _ => unimplemented!(),
    }
}

fn boundary_analysis(bril_op: BrilOp) -> String {
    let bop = bril_op.op.to_string();
    match bril_op.input_types.as_ref() {
        // both side of operand need a rule
        [Some(_), Some(_)] => format!(
            "
        (rule ((= true (is_inv_operand theta operand)) 
                (= false (is_inv_expr theta expr))
                (= expr ({bop} ty operand b)))
            ((boundary_operand theta operand)) :ruleset boundary-analyses)
        
        (rule ((= true (is_inv_operand theta operand)) 
                (= false (is_inv_expr theta expr))
                (= expr ({bop} ty a operand)))
            ((boundary_operand theta operand)) :ruleset boundary-analyses)
        "
        ),
        [Some(_), None] => String::new(), // unary operator should not be on boundary.
        _ => unimplemented!(),
    }
}

pub(crate) fn loop_invariant_detection() -> String {
    let mut res = vec![include_str!("loop_invariant.egg").to_string()];

    for bril_op in BRIL_OPS {
        res.push(inv_ops_detection(bril_op.clone()));
        res.push(boundary_analysis(bril_op));
    }

    // delete after bool-= is added to egglog
    res.push(
        "(rule ((find_inv_expr theta (beq ty a b)))
        ((find_inv_operand theta a) (find_inv_operand theta b)) :ruleset fast-analyses)
    
    (rule ((= true (is_inv_operand body a))
            (= true (is_inv_operand body b))
            (find_inv_expr body expr)
            (= expr (beq ty a b)))
        ((set (is_inv_expr body expr) true)) :ruleset fast-analyses)

    (rule ((= true (is_inv_operand theta operand)) 
        (= false (is_inv_expr theta expr))
        (= expr (beq ty operand b)))
        ((boundary_operand theta operand)) :ruleset boundary-analyses)

    (rule ((= true (is_inv_operand theta operand)) 
            (= false (is_inv_expr theta expr))
            (= expr (beq ty a operand)))
        ((boundary_operand theta operand)) :ruleset boundary-analyses)
        "
        .to_string(),
    );

    res.join("\n")
}
