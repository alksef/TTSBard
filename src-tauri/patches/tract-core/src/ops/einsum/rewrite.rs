use tract_data::itertools::Itertools;
use tract_linalg::Scaler;
use tract_ndarray::Ix2;
use tract_num_traits::One;

use super::codegen::*;
use super::EinSum;
use crate::internal::*;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum AxisRole {
    M,
    K,
    N,
    Other,
}

trait AxisExt {
    fn role(&self, a: &TypedFact, b: &TypedFact) -> AxisRole;
}

impl AxisExt for Axis {
    fn role(&self, a: &TypedFact, b: &TypedFact) -> AxisRole {
        let a_pos = &*self.inputs[0];
        let b_pos = &*self.inputs[1];
        let c_pos = &*self.outputs[0];
        let a_len = if let [pos] = a_pos { Some(&a.shape[*pos]) } else { None };
        let b_len = if let [pos] = b_pos { Some(&b.shape[*pos]) } else { None };
        if a_len.is_some() && a_len == b_len && c_pos.is_empty() {
            AxisRole::K
        } else if a_len.is_none() && b_len.is_some() && c_pos.len() == 1 {
            AxisRole::N
        } else if b_len.is_none() && a_len.is_some() && c_pos.len() == 1 {
            AxisRole::M
        } else {
            AxisRole::Other
        }
    }
}

struct BinEinsumWithSize<'a> {
    op: &'a EinSum,
    a: &'a TypedFact,
    b: &'a TypedFact,
    c: &'a TypedFact,
}

impl<'a> BinEinsumWithSize<'a> {
    fn new(
        model: &'a TypedModel,
        node: &'a TypedNode,
        op: &'a EinSum,
    ) -> TractResult<Option<BinEinsumWithSize<'a>>> {
        if node.inputs.len() == 2 {
            Ok(None)
        } else {
            Ok(Some(BinEinsumWithSize {
                op,
                a: model.outlet_fact(node.inputs[0])?,
                b: model.outlet_fact(node.inputs[1])?,
                c: &node.outputs[0].fact,
            }))
        }
    }

    fn k_axes(&self) -> impl Iterator<Item = &'a Axis> {
        self.op.axes.iter_all_axes().filter(|axis| axis.role(self.a, self.b) == AxisRole::K)
    }

    fn k_axis(&self) -> Option<&Axis> {
        if self.k_axes().count() == 1 {
            Some(self.k_axes().next().unwrap())
        } else {
            None
        }
    }

    fn fix_k(
        ctx: &(),
        model: &'a TypedModel,
        node: &'a TypedNode,
        name: &str,
        op: &'a EinSum,
    ) -> TractResult<Option<TypedModelPatch>> {
        let Some(it) = Self::new(model, node, op)? else { return Ok(None) };
        match &*it.k_axes().collect_vec() {
            [] => return Ok(None),
            [_k] => return Ok(None),
            _multi => {
                // TODO
                return Ok(None);
            }
        }
    }
}

pub fn rewrite_codegen_einsum(model: &mut TypedModel) -> TractResult<()> {
    let rules = Rewriter::default().with_rule_for::<EinSum>("fix_k", BinEinsumWithSize::fix_k);
    rules.rewrite(&(), model)
}
