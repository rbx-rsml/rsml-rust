mod tuple_annotations;
use tuple_annotations::TUPLE_ANNOTATIONS;

use super::Datatype;

pub struct Tuple {
    pub name: Option<String>,
    pub parent: Option<usize>,
    data: Vec<Datatype>
}

impl Tuple {
    pub fn new(name: Option<String>, parent: Option<usize>) -> Self {
        Self {
            name,
            parent,
            data: vec![]
        }
    }

    pub fn coerce_to_datatype(&self) -> Datatype {
        let tuple_data = &self.data;
    
        if let Some(tuple_name) = &self.name {
            if let Some(annotation_function) = TUPLE_ANNOTATIONS.get(&tuple_name.to_lowercase()) {
                return annotation_function(tuple_data)
            }
        }
    
        let tuple_length = tuple_data.len();
    
    
        if tuple_length == 0 { Datatype::None }
        else if tuple_length == 1 { tuple_data[0].clone() }
        else { Datatype::TupleData(tuple_data.clone()) }
    }

    pub fn push(&mut self, datatype: Datatype) {
        self.data.push(datatype);
    }
}