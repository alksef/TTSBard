use std::fmt::{Debug, Display};
use std::ops::Deref;

use crate::internal::*;

#[derive(Debug, new)]
pub struct Wirer<'m, 's, F, O, M>(&'m mut M, &'s str, PhantomData<(F, O)>);

impl<'m, 's, F, O, M> SpecialOps<F, O> for Wirer<'m, 's, F, O, M>
where
    M: SpecialOps<F, O>,
{
    fn create_dummy(&self) -> O {
        self.0.create_dummy()
    }

    fn create_source(&self, fact: F) -> O {
        self.0.create_source(fact)
    }

    fn is_source(op: &O) -> bool {
        M::is_source(op)
    }

    fn wire_node(
        &mut self,
        name: impl Into<String>,
        op: impl Into<O>,
        inputs: &[OutletId],
    ) -> TractResult<TVec<OutletId>> {
        self.0.wire_node(self.prefix(name), op, inputs)
    }
}

impl<'m, 's, F, O> Wirer<'m, 's, F, O, ModelPatch<F, O>>
where
    F: Fact + Clone + 'static,
    O: Display + Debug + AsRef<dyn Op> + AsMut<dyn Op> + Clone + 'static,
    Graph<F, O>: SpecialOps<F, O>,
{
    pub fn wire_node(
        &mut self,
        name: impl Into<String>,
        op: impl Into<O>,
        inputs: &[OutletId],
    ) -> TractResult<TVec<OutletId>> {
        let name = self.prefix(name);
        self.0.wire_node(name, op, inputs)
    }
}

impl<'m, 's, F, O> Wirer<'m, 's, F, O, TypedModelPatch>
where
    F: Fact + Clone + 'static,
    O: Display + Debug + AsRef<dyn Op> + AsMut<dyn Op> + Clone + 'static,
    Graph<F, O>: SpecialOps<F, O>,
{
    pub fn add_const(
        &mut self,
        name: impl Into<String>,
        t: impl IntoArcTensor,
    ) -> TractResult<OutletId> {
        let name = self.prefix(name);
        self.0.add_const(name, t)
    }
}

// impl<'m, 's, F, O> Wirer<'m, 's, F, O, ModelPatch<F, O>>
// where
//     F: Fact + Clone + 'static,
//     O: Display + Debug + AsRef<dyn Op> + AsMut<dyn Op> + Clone + 'static,
//     Graph<F, O>: SpecialOps<F, O>,
// {
//     pub fn add_const(
//         &mut self,
//         name: impl Into<String>,
//         t: impl IntoArcTensor,
//     ) -> TractResult<OutletId> {
//         self.0.deref_mut().add_const(self.prefix(name), t)
//     }
// }

impl<'m, 's, F, O, M> Wirer<'m, 's, F, O, M> {
    pub fn name(&self) -> &'s str {
        self.1
    }

    fn prefix(&self, name: impl Into<String>) -> String {
        let name = name.into();
        if self.1.len() == 0 {
            name
        } else {
            if name.len() == 0 {
                self.1.to_string()
            } else {
                format!("{}.{}", self.1, name)
            }
        }
    }
}

impl<'m, 's> Wirer<'m, 's, TypedFact, Box<dyn TypedOp>, TypedModel> {
    pub fn add_const(
        &mut self,
        name: impl Into<String>,
        t: impl IntoArcTensor,
    ) -> TractResult<OutletId> {
        self.0.add_const(self.prefix(name), t)
    }
}

pub type TypedWirer<'m, 's> = Wirer<'m, 's, TypedFact, Box<dyn TypedOp>, TypedModel>;

impl<'m, 's> Deref for TypedWirer<'m, 's> {
    type Target = TypedModel;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'m, 's> AsRef<TypedModel> for TypedWirer<'m, 's> {
    fn as_ref(&self) -> &TypedModel {
        &self.0
    }
}

impl<'m, 's> AsMut<TypedModel> for TypedWirer<'m, 's> {
    fn as_mut(&mut self) -> &mut TypedModel {
        &mut self.0
    }
}

pub trait WithPrefix<'m, 's> {
    type Target;
    fn with_prefix(&'m mut self, name: &'s str) -> Self::Target;
}

impl<'m, 's> WithPrefix<'m, 's> for TypedModel {
    type Target = Wirer<'m, 's, TypedFact, Box<dyn TypedOp>, TypedModel>;
    fn with_prefix(&'m mut self, name: &'s str) -> Self::Target {
        Wirer::new(self, name)
    }
}

impl<'m, 's> WithPrefix<'m, 's> for TypedModelPatch {
    type Target = Wirer<'m, 's, TypedFact, Box<dyn TypedOp>, TypedModelPatch>;
    fn with_prefix(&'m mut self, name: &'s str) -> Self::Target {
        Wirer::new(self, name)
    }
}
