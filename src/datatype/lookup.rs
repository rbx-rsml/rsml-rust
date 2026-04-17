use super::Datatype;

pub trait StaticLookup {
    fn resolve_static(&self, name: &str) -> Datatype;
    fn resolve_dynamic(&self, name: &str) -> Datatype;
}
