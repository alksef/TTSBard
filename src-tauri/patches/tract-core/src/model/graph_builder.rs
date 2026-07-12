use std::fmt::{Debug, Display};

use crate::internal::*;

pub trait SpecialOps<F, O> {
    fn create_dummy(&self) -> O;
    fn create_source(&self, fact: F) -> O;
    fn is_source(op: &O) -> bool;
    fn wire_node(
        &mut self,
        name: impl Into<String>,
        op: impl Into<O>,
        inputs: &[OutletId],
    ) -> TractResult<TVec<OutletId>>;
}

pub struct GraphBuilder<F, O>
where
    F: Fact + Clone + 'static,
    O: Debug + Display + AsRef<dyn Op> + AsMut<dyn Op> + Clone + 'static,
    Graph<F, O>: SpecialOps<F, O>,
{
    model: Graph<F, O>,
}

impl<F, O> GraphBuilder<F, O>
where
    F: Fact + Clone + 'static,
    O: Debug+ Display + AsRef<dyn Op> + AsMut<dyn Op> + Clone + 'static,
    Graph<F, O>: SpecialOps<F, O>,
{
    pub fn add_source(&mut self, name: impl Into<String>, fact: F) -> TractResult<OutletId> {
        let source = self.model.create_source(fact.clone());
        let id = self.model.add_node(name, source, tvec!(fact))?;
        let id = OutletId::new(id, 0);
        self.model.inputs.push(id);
        Ok(id)
    }

    pub fn add_node(
        &mut self,
        name: impl Into<String>,
        op: impl Into<O>,
        output_facts: TVec<F>,
    ) -> TractResult<usize> {
        let op = op.into();
        let name = name.into();
        let id = self.model.nodes.len();
        let outputs =
            output_facts.into_iter().map(|fact| Outlet { fact, successors: tvec!() }).collect();
        let node = Node { id, name, op, inputs: vec![], outputs };
        self.model.nodes.push(node);
        Ok(id)
    }

    /// Connect a node outlet to a node inlet.
    pub fn add_edge(&mut self, outlet: OutletId, inlet: InletId) -> TractResult<()> {
        if let Some(previous) = self.model.nodes[inlet.node].inputs.get(inlet.slot).cloned() {
            self.model.nodes[previous.node].outputs[previous.slot]
                .successors
                .retain(|&mut succ| succ != inlet);
        }
        {
            let prec = &mut self.model.nodes[outlet.node];
            prec.outputs[outlet.slot].successors.push(inlet);
        }
        let succ = &mut self.model.nodes[inlet.node];
        #[allow(clippy::comparison_chain)]
        if inlet.slot == succ.inputs.len() {
            succ.inputs.push(outlet);
        } else if inlet.slot < succ.inputs.len() {
            succ.inputs[inlet.slot] = outlet;
        } else {
            bail!("Edges must be added in order and consecutive. Trying to connect input {:?} of node {:?} ", inlet.slot, succ)
        }
        Ok(())
    }

}
