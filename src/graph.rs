use std::collections::HashMap;

pub type Graph = petgraph::Graph<&'static str, &'static str>;

struct Id(&'static str);

pub trait Resource {
    fn id(&self) -> Id;
}
