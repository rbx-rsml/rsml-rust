use crate::datatype::Datatype;

pub trait StaticLookup {
    fn resolve_static(&self, name: &str) -> Datatype;
    fn resolve_dynamic(&self, name: &str) -> Datatype;
    fn resolve_macro_arg(&self, _name: &str, _key: Option<&str>) -> Option<Datatype> {
        None
    }
}
