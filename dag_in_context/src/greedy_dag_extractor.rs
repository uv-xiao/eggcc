use egglog::*;
use egraph_serialize::{ClassId, EGraph, NodeId};
use indexmap::*;
use ordered_float::NotNan;
use rustc_hash::FxHashMap;
use std::collections::{HashMap, HashSet, VecDeque};

pub(crate) struct Extractor<'a> {
    pub(crate) cm: &'a dyn CostModel,
    pub(crate) termdag: &'a mut TermDag,
    #[allow(dead_code)]
    pub(crate) correspondence: HashMap<Term, NodeId>,
    pub(crate) egraph: &'a EGraph,
    costs: FxHashMap<ClassId, CostSet>,
    /// A set of names of functions that are unextractable
    unextractables: HashSet<String>,
}

impl<'a> Extractor<'a> {
    pub(crate) fn eclass_of(&self, term: &Term) -> ClassId {
        let term_enode = self
            .correspondence
            .get(term)
            .unwrap_or_else(|| panic!("Failed to find correspondence for term {:?}", term));
        self.egraph.nodes.get(term_enode).unwrap().eclass.clone()
    }

    pub(crate) fn new(
        cm: &'a dyn CostModel,
        termdag: &'a mut TermDag,
        correspondence: HashMap<Term, NodeId>,
        egraph: &'a EGraph,
        unextractables: HashSet<String>,
    ) -> Self {
        Extractor {
            cm,
            termdag,
            correspondence,
            egraph,
            costs: FxHashMap::<ClassId, CostSet>::with_capacity_and_hasher(
                egraph.classes().len(),
                Default::default(),
            ),
            unextractables,
        }
    }
}

fn get_root(egraph: &egraph_serialize::EGraph) -> NodeId {
    let mut root_nodes = egraph
        .nodes
        .iter()
        .filter(|(_nid, node)| node.op == "Program");
    let res = root_nodes.next().unwrap();
    assert!(root_nodes.next().is_none());
    res.0.clone()
}

pub fn serialized_egraph(
    egglog_egraph: egglog::EGraph,
) -> (egraph_serialize::EGraph, HashSet<String>) {
    let config = SerializeConfig::default();
    let egraph = egglog_egraph.serialize(config);

    let unextractables = egglog_egraph
        .functions
        .iter()
        .filter_map(|(name, func)| {
            if func.is_extractable() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect();
    (egraph, unextractables)
}

type Cost = NotNan<f64>;

#[derive(Clone)]
pub struct CostSet {
    pub total: Cost,
    // TODO perhaps more efficient as
    // persistent data structure?
    pub costs: HashMap<ClassId, Cost>,
    pub term: Term,
}

fn build_parent_index(egraph: &egraph_serialize::EGraph) -> IndexMap<ClassId, Vec<NodeId>> {
    let mut parents = IndexMap::<ClassId, Vec<NodeId>>::with_capacity(egraph.classes().len());
    let n2c = |nid: &NodeId| egraph.nid_to_cid(nid);

    for class in egraph.classes().values() {
        parents.insert(class.id.clone(), Vec::new());
    }

    for class in egraph.classes().values() {
        for node in &class.nodes {
            for c in &egraph[node].children {
                // compute parents of this enode
                parents[n2c(c)].push(node.clone());
            }
        }
    }
    parents
}

fn initialize_worklist(egraph: &egraph_serialize::EGraph) -> UniqueQueue<NodeId> {
    let mut analysis_pending = UniqueQueue::default();
    for class in egraph.classes().values() {
        for node in &class.nodes {
            // start the analysis from leaves
            if egraph[node].is_leaf() {
                analysis_pending.insert(node.clone());
            }
        }
    }
    analysis_pending
}

fn get_term(op: &str, cost_sets: &[&CostSet], termdag: &mut TermDag) -> Term {
    if cost_sets.is_empty() {
        if op.starts_with('\"') {
            return termdag.lit(ast::Literal::String(op[1..op.len() - 1].into()));
        }
        if let Ok(n) = op.parse::<i64>() {
            return termdag.lit(ast::Literal::Int(n));
        }
        if op == "true" {
            return termdag.lit(ast::Literal::Bool(true));
        }
        if op == "false" {
            return termdag.lit(ast::Literal::Bool(false));
        }
    }
    termdag.app(
        op.into(),
        cost_sets.iter().map(|cs| cs.term.clone()).collect(),
    )
}

/// Handles the edge case of cycles, then calls get_node_cost
fn calculate_cost_set(
    egraph: &egraph_serialize::EGraph,
    node_id: NodeId,
    extractor: &mut Extractor,
) -> CostSet {
    let node = &egraph[&node_id];
    let cid = egraph.nid_to_cid(&node_id);

    let children_classes = node
        .children
        .iter()
        .map(|c| egraph.nid_to_cid(c).clone())
        .collect::<Vec<ClassId>>();

    let child_cost_sets: Vec<_> = children_classes
        .iter()
        .map(|c| extractor.costs.get(c).unwrap())
        .collect();

    // cycle detection
    if child_cost_sets.iter().any(|cs| cs.costs.contains_key(cid)) {
        return CostSet {
            costs: Default::default(),
            total: std::f64::INFINITY.try_into().unwrap(),
            // returns junk children since this cost set is guaranteed to not be selected.
            term: extractor.termdag.app(node.op.as_str().into(), vec![]),
        };
    }

    let mut total = extractor.cm.get_op_cost(&node.op);
    let mut costs = HashMap::from([(cid.clone(), total)]);
    let term = get_term(&node.op, &child_cost_sets, extractor.termdag);

    let unshared_children = extractor.cm.unshared_children(&node.op);
    if !extractor.cm.ignore_children(&node.op) {
        for (i, child_set) in child_cost_sets.iter().enumerate() {
            if unshared_children.contains(&i) {
                // don't add to the cost set, but do add to the total
                total += child_set.total;
            } else {
                for (child_cid, child_cost) in &child_set.costs {
                    // it was already present in the set
                    if let Some(existing) = costs.insert(child_cid.clone(), *child_cost) {
                        assert_eq!(
                            existing, *child_cost,
                            "Two different costs found for the same child enode!"
                        );
                    } else {
                        total += child_cost;
                    }
                }
            }
        }
    }

    CostSet { total, costs, term }
}

pub fn extract(
    egraph: &egraph_serialize::EGraph,
    unextractables: HashSet<String>,
    termdag: &mut TermDag,
    cost_model: impl CostModel,
) -> CostSet {
    let extractor_not_linear = &mut Extractor::new(
        &cost_model,
        termdag,
        Default::default(),
        egraph,
        unextractables,
    );

    let res = extract_without_linearity(extractor_not_linear);
    // TODO use effectul regions to extract maintaining linearity
    let _effectful_regions = extractor_not_linear.find_effectful_nodes_in_program(&res.term);

    res
}

/// Perform a greedy extraction of the DAG, without considering linearity.
/// This uses the "fast_greedy_dag" algorithm from the extraction gym.
pub fn extract_without_linearity(extractor: &mut Extractor) -> CostSet {
    let n2c = |nid: &NodeId| extractor.egraph.nid_to_cid(nid);
    let parents = build_parent_index(extractor.egraph);
    let mut worklist = initialize_worklist(extractor.egraph);

    while let Some(node_id) = worklist.pop() {
        let class_id = n2c(&node_id);
        let node = &extractor.egraph[&node_id];
        if extractor.unextractables.contains(&node.op) {
            continue;
        }
        if node
            .children
            .iter()
            .all(|c| extractor.costs.contains_key(n2c(c)))
        {
            let lookup = extractor.costs.get(class_id);
            let mut prev_cost: Cost = std::f64::INFINITY.try_into().unwrap();
            if lookup.is_some() {
                prev_cost = lookup.unwrap().total;
            }

            let cost_set = calculate_cost_set(extractor.egraph, node_id.clone(), extractor);
            if cost_set.total < prev_cost {
                extractor.costs.insert(class_id.clone(), cost_set);
                worklist.extend(parents[class_id].iter().cloned());
            }
        }
    }

    let mut root_eclasses = extractor.egraph.root_eclasses.clone();
    root_eclasses.sort();
    root_eclasses.dedup();

    let root = get_root(extractor.egraph);
    extractor.costs.get(&n2c(&root)).unwrap().clone()
}

pub trait CostModel {
    /// TODO: we could do better with type info
    fn get_op_cost(&self, op: &str) -> Cost;

    /// if true, the op's children are ignored in calculating the cost
    fn ignore_children(&self, op: &str) -> bool;

    /// returns a slice of indices into the children vec
    /// count the cost of these children, but don't add the nodes they depend on to the set
    fn unshared_children(&self, op: &str) -> &'static [usize];
}

pub struct DefaultCostModel;

impl CostModel for DefaultCostModel {
    fn get_op_cost(&self, op: &str) -> Cost {
        match op {
            // Leaves
            "Const" => 1.,
            "Arg" => 0.,
            _ if op.parse::<i64>().is_ok() || op.starts_with('"') => 0.,
            "true" | "false" | "()" => 0.,
            // Lists
            "Empty" | "Single" | "Concat" | "Get" | "Nil" | "Cons" => 0.,
            // Types
            "IntT" | "BoolT" | "PointerT" | "StateT" => 0.,
            "Base" | "TupleT" | "TNil" | "TCons" => 0.,
            "Int" | "Bool" => 0.,
            // Algebra
            "Add" | "PtrAdd" | "Sub" | "And" | "Or" | "Not" => 10.,
            "Mul" => 30.,
            "Div" => 50.,
            // Comparisons
            "Eq" | "LessThan" | "GreaterThan" | "LessEq" | "GreaterEq" => 10.,
            // Effects
            "Print" | "Write" | "Load" => 50.,
            "Alloc" | "Free" => 100.,
            "Call" => 1000., // TODO: we could make this more accurate
            // Control
            "Program" | "Function" => 1.,
            "DoWhile" => 100., // TODO: we could make this more accurate
            "If" | "Switch" => 50.,
            // Unreachable
            "HasType" | "HasArgType" | "ContextOf" | "NoContext" | "ExpectType" => 0.,
            "ExprIsPure" | "ListExprIsPure" | "BinaryOpIsPure" | "UnaryOpIsPure" => 0.,
            "IsLeaf" | "BodyContainsExpr" | "ScopeContext" => 0.,
            "Region" | "Full" | "IntI" | "BoolI" => 0.,
            // Schema
            "Bop" | "Uop" | "Top" => 0.,
            "InContext" => 0.,
            _ if self.ignore_children(op) => 0.,
            _ => panic!("no cost for {op}"),
        }
        .try_into()
        .unwrap()
    }

    fn ignore_children(&self, op: &str) -> bool {
        matches!(op, "InLoop" | "NoContext" | "InSwitch" | "InIf")
    }

    fn unshared_children(&self, op: &str) -> &'static [usize] {
        match op {
            "DoWhile" => &[1],
            "Function" => &[3],
            "If" => &[2, 3],
            "Switch" => &[2], // TODO: Switch branches can share nodes
            _ => &[],
        }
    }
}

/** A data structure to maintain a queue of unique elements.

Notably, insert/pop operations have O(1) expected amortized runtime complexity.

Thanks @Bastacyclop for the implementation!
*/
#[derive(Clone)]
pub(crate) struct UniqueQueue<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    set: HashSet<T>,
    queue: VecDeque<T>,
}

impl<T> Default for UniqueQueue<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    fn default() -> Self {
        UniqueQueue {
            set: Default::default(),
            queue: Default::default(),
        }
    }
}

impl<T> UniqueQueue<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    pub fn insert(&mut self, t: T) {
        if self.set.insert(t.clone()) {
            self.queue.push_back(t);
        }
    }

    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        for t in iter.into_iter() {
            self.insert(t);
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        let res = self.queue.pop_front();
        res.as_ref().map(|t| self.set.remove(t));
        res
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        let r = self.queue.is_empty();
        debug_assert_eq!(r, self.set.is_empty());
        r
    }
}
